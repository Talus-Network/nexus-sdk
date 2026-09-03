//! Test only functions layered onto published Nexus modules.

use {
    anyhow::{anyhow, bail, Context as _, Result},
    move_binary_format::{
        file_format::{
            AddressIdentifierIndex,
            Bytecode,
            CodeUnit,
            Constant,
            ConstantPoolIndex,
            DatatypeHandle,
            DatatypeHandleIndex,
            EnumDefInstantiation,
            EnumDefInstantiationIndex,
            EnumDefinitionIndex,
            FieldDefinition,
            FieldHandle,
            FieldHandleIndex,
            FieldInstantiation,
            FieldInstantiationIndex,
            FunctionDefinition,
            FunctionHandle,
            FunctionHandleIndex,
            FunctionInstantiation,
            FunctionInstantiationIndex,
            IdentifierIndex,
            ModuleHandle,
            ModuleHandleIndex,
            Signature,
            SignatureIndex,
            SignatureToken,
            StructDefInstantiation,
            StructDefInstantiationIndex,
            StructDefinitionIndex,
            StructFieldInformation,
            TableIndex,
            VariantHandle,
            VariantHandleIndex,
            VariantInstantiationHandle,
            VariantInstantiationHandleIndex,
            VariantJumpTable,
        },
        CompiledModule,
    },
    move_bytecode_verifier::verify_module_unmetered,
    std::collections::{BTreeMap, BTreeSet},
};

/// Adds the named functions from a source compiled interface to the matching
/// published module without changing any published definition.
pub(super) fn add_test_functions(
    published: CompiledModule,
    interface: &CompiledModule,
    function_names: &BTreeSet<String>,
) -> Result<CompiledModule> {
    if published.self_id() != interface.self_id() {
        bail!(
            "cannot overlay interface module '{}' on published module '{}'",
            interface.self_id(),
            published.self_id()
        );
    }

    let mut linker = TestFunctionLinker::new(interface, published, function_names.clone());
    linker.add_functions()?;
    let linked = linker.finish();
    verify_module_unmetered(&linked).map_err(|error| {
        anyhow!(
            "test overlay for '{}' is invalid: {error}",
            linked.self_id()
        )
    })?;
    Ok(linked)
}

struct TestFunctionLinker<'a> {
    source: &'a CompiledModule,
    target: CompiledModule,
    allowed_new_functions: BTreeSet<String>,
    module_handles: Vec<Option<ModuleHandleIndex>>,
    datatype_handles: Vec<Option<DatatypeHandleIndex>>,
    function_handles: Vec<Option<FunctionHandleIndex>>,
    field_handles: Vec<Option<FieldHandleIndex>>,
    struct_defs: Vec<Option<StructDefinitionIndex>>,
    enum_defs: Vec<Option<EnumDefinitionIndex>>,
    signatures: Vec<Option<SignatureIndex>>,
    constants: Vec<Option<ConstantPoolIndex>>,
    struct_instantiations: Vec<Option<StructDefInstantiationIndex>>,
    function_instantiations: Vec<Option<FunctionInstantiationIndex>>,
    field_instantiations: Vec<Option<FieldInstantiationIndex>>,
    enum_instantiations: Vec<Option<EnumDefInstantiationIndex>>,
    variant_handles: Vec<Option<VariantHandleIndex>>,
    variant_instantiations: Vec<Option<VariantInstantiationHandleIndex>>,
}

impl<'a> TestFunctionLinker<'a> {
    fn new(
        source: &'a CompiledModule,
        target: CompiledModule,
        allowed_new_functions: BTreeSet<String>,
    ) -> Self {
        Self {
            module_handles: vec![None; source.module_handles.len()],
            datatype_handles: vec![None; source.datatype_handles.len()],
            function_handles: vec![None; source.function_handles.len()],
            field_handles: vec![None; source.field_handles.len()],
            struct_defs: vec![None; source.struct_defs.len()],
            enum_defs: vec![None; source.enum_defs.len()],
            signatures: vec![None; source.signatures.len()],
            constants: vec![None; source.constant_pool.len()],
            struct_instantiations: vec![None; source.struct_def_instantiations.len()],
            function_instantiations: vec![None; source.function_instantiations.len()],
            field_instantiations: vec![None; source.field_instantiations.len()],
            enum_instantiations: vec![None; source.enum_def_instantiations.len()],
            variant_handles: vec![None; source.variant_handles.len()],
            variant_instantiations: vec![None; source.variant_instantiation_handles.len()],
            source,
            target,
            allowed_new_functions,
        }
    }

    fn finish(mut self) -> CompiledModule {
        self.target.publishable = false;
        self.target.version = self.target.version.max(self.source.version);
        self.target
    }

    fn add_functions(&mut self) -> Result<()> {
        let source_definitions = self.source_function_definitions()?;

        for (_, definition) in &source_definitions {
            self.map_function_handle(definition.function)?;
        }

        for (name, definition) in source_definitions {
            let function = self.map_function_handle(definition.function)?;
            let acquires_global_resources = definition
                .acquires_global_resources
                .into_iter()
                .map(|index| self.map_struct_definition(index))
                .collect::<Result<Vec<_>>>()?;
            let code = definition
                .code
                .map(|code| self.map_code_unit(code))
                .transpose()?;

            self.target.function_defs.push(FunctionDefinition {
                function,
                visibility: definition.visibility,
                is_entry: definition.is_entry,
                acquires_global_resources,
                code,
            });

            self.allowed_new_functions.remove(&name);
        }

        if !self.allowed_new_functions.is_empty() {
            bail!(
                "compiled interface '{}' does not contain test functions: {}",
                self.source.self_id(),
                self.allowed_new_functions
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        Ok(())
    }

    fn source_function_definitions(&self) -> Result<Vec<(String, FunctionDefinition)>> {
        let published_names = local_function_definitions(&self.target)?;
        let source_definitions = local_function_definitions(self.source)?;
        let mut selected = Vec::with_capacity(self.allowed_new_functions.len());

        for name in &self.allowed_new_functions {
            if published_names.contains_key(name) {
                bail!(
                    "test extension cannot redefine published function '{}::{}'",
                    self.target.self_id(),
                    name
                );
            }
            let definition = source_definitions.get(name).ok_or_else(|| {
                anyhow!(
                    "test extension function '{}::{}' was not emitted by the Move compiler",
                    self.source.self_id(),
                    name
                )
            })?;
            selected.push((name.clone(), (*definition).clone()));
        }
        Ok(selected)
    }

    fn map_identifier(&mut self, index: IdentifierIndex) -> Result<IdentifierIndex> {
        let value = self
            .source
            .identifiers
            .get(index.0 as usize)
            .with_context(|| format!("invalid source identifier index {index}"))?
            .clone();
        if let Some(index) = self
            .target
            .identifiers
            .iter()
            .position(|item| item == &value)
        {
            return make_index(index, "identifier", IdentifierIndex::new);
        }
        let index = make_index(
            self.target.identifiers.len(),
            "identifier",
            IdentifierIndex::new,
        )?;
        self.target.identifiers.push(value);
        Ok(index)
    }

    fn map_address(&mut self, index: AddressIdentifierIndex) -> Result<AddressIdentifierIndex> {
        let value = *self
            .source
            .address_identifiers
            .get(index.0 as usize)
            .with_context(|| format!("invalid source address index {index}"))?;
        if let Some(index) = self
            .target
            .address_identifiers
            .iter()
            .position(|item| item == &value)
        {
            return make_index(index, "address", AddressIdentifierIndex::new);
        }
        let index = make_index(
            self.target.address_identifiers.len(),
            "address",
            AddressIdentifierIndex::new,
        )?;
        self.target.address_identifiers.push(value);
        Ok(index)
    }

    fn map_module_handle(&mut self, index: ModuleHandleIndex) -> Result<ModuleHandleIndex> {
        if let Some(mapped) = cached(&self.module_handles, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .module_handles
            .get(index.0 as usize)
            .with_context(|| format!("invalid source module handle index {index}"))?
            .clone();
        let mapped = ModuleHandle {
            address: self.map_address(source.address)?,
            name: self.map_identifier(source.name)?,
        };
        let target_index = if let Some(position) = self
            .target
            .module_handles
            .iter()
            .position(|item| item == &mapped)
        {
            make_index(position, "module handle", ModuleHandleIndex::new)?
        } else {
            let target_index = make_index(
                self.target.module_handles.len(),
                "module handle",
                ModuleHandleIndex::new,
            )?;
            self.target.module_handles.push(mapped);
            target_index
        };
        self.module_handles[index.0 as usize] = Some(target_index);
        Ok(target_index)
    }

    fn map_datatype_handle(&mut self, index: DatatypeHandleIndex) -> Result<DatatypeHandleIndex> {
        if let Some(mapped) = cached(&self.datatype_handles, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .datatype_handles
            .get(index.0 as usize)
            .with_context(|| format!("invalid source datatype handle index {index}"))?
            .clone();
        let mapped = DatatypeHandle {
            module: self.map_module_handle(source.module)?,
            name: self.map_identifier(source.name)?,
            abilities: source.abilities,
            type_parameters: source.type_parameters,
        };

        let same_name = self
            .target
            .datatype_handles
            .iter()
            .position(|item| item.module == mapped.module && item.name == mapped.name);
        let target_index = if let Some(position) = same_name {
            if self.target.datatype_handles[position] != mapped {
                bail!(
                    "datatype ABI mismatch while linking test extension for '{}'",
                    self.target.self_id()
                );
            }
            make_index(position, "datatype handle", DatatypeHandleIndex::new)?
        } else {
            if mapped.module == self.target.self_module_handle_idx {
                bail!(
                    "test extension references a datatype missing from published module '{}'",
                    self.target.self_id()
                );
            }
            let target_index = make_index(
                self.target.datatype_handles.len(),
                "datatype handle",
                DatatypeHandleIndex::new,
            )?;
            self.target.datatype_handles.push(mapped);
            target_index
        };
        self.datatype_handles[index.0 as usize] = Some(target_index);
        Ok(target_index)
    }

    fn map_signature_token(&mut self, token: SignatureToken) -> Result<SignatureToken> {
        use SignatureToken::*;

        Ok(match token {
            Vector(inner) => Vector(Box::new(self.map_signature_token(*inner)?)),
            Datatype(index) => Datatype(self.map_datatype_handle(index)?),
            DatatypeInstantiation(instantiation) => {
                let (index, parameters) = *instantiation;
                DatatypeInstantiation(Box::new((
                    self.map_datatype_handle(index)?,
                    parameters
                        .into_iter()
                        .map(|token| self.map_signature_token(token))
                        .collect::<Result<Vec<_>>>()?,
                )))
            }
            Reference(inner) => Reference(Box::new(self.map_signature_token(*inner)?)),
            MutableReference(inner) => {
                MutableReference(Box::new(self.map_signature_token(*inner)?))
            }
            Bool | U8 | U16 | U32 | U64 | U128 | U256 | Address | Signer | TypeParameter(_) => {
                token
            }
        })
    }

    fn map_signature(&mut self, index: SignatureIndex) -> Result<SignatureIndex> {
        if let Some(mapped) = cached(&self.signatures, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .signatures
            .get(index.0 as usize)
            .with_context(|| format!("invalid source signature index {index}"))?
            .clone();
        let mapped = Signature(
            source
                .0
                .into_iter()
                .map(|token| self.map_signature_token(token))
                .collect::<Result<Vec<_>>>()?,
        );
        let target_index = if let Some(position) = self
            .target
            .signatures
            .iter()
            .position(|item| item == &mapped)
        {
            make_index(position, "signature", SignatureIndex::new)?
        } else {
            let target_index = make_index(
                self.target.signatures.len(),
                "signature",
                SignatureIndex::new,
            )?;
            self.target.signatures.push(mapped);
            target_index
        };
        self.signatures[index.0 as usize] = Some(target_index);
        Ok(target_index)
    }

    fn map_function_handle(&mut self, index: FunctionHandleIndex) -> Result<FunctionHandleIndex> {
        if let Some(mapped) = cached(&self.function_handles, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .function_handles
            .get(index.0 as usize)
            .with_context(|| format!("invalid source function handle index {index}"))?
            .clone();
        let mapped = FunctionHandle {
            module: self.map_module_handle(source.module)?,
            name: self.map_identifier(source.name)?,
            parameters: self.map_signature(source.parameters)?,
            return_: self.map_signature(source.return_)?,
            type_parameters: source.type_parameters,
        };
        let same_name = self
            .target
            .function_handles
            .iter()
            .position(|item| item.module == mapped.module && item.name == mapped.name);
        let target_index = if let Some(position) = same_name {
            if self.target.function_handles[position] != mapped {
                bail!(
                    "function ABI mismatch while linking test extension for '{}'",
                    self.target.self_id()
                );
            }
            make_index(position, "function handle", FunctionHandleIndex::new)?
        } else {
            if mapped.module == self.target.self_module_handle_idx {
                let name = self.target.identifiers[mapped.name.0 as usize].as_str();
                if !self.allowed_new_functions.contains(name) {
                    bail!(
                        "test function '{}::{}' has no selected definition",
                        self.target.self_id(),
                        name
                    );
                }
            }
            let target_index = make_index(
                self.target.function_handles.len(),
                "function handle",
                FunctionHandleIndex::new,
            )?;
            self.target.function_handles.push(mapped);
            target_index
        };
        self.function_handles[index.0 as usize] = Some(target_index);
        Ok(target_index)
    }

    fn map_struct_definition(
        &mut self,
        index: StructDefinitionIndex,
    ) -> Result<StructDefinitionIndex> {
        if let Some(mapped) = cached(&self.struct_defs, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .struct_defs
            .get(index.0 as usize)
            .with_context(|| format!("invalid source struct definition index {index}"))?
            .clone();
        let handle = self.map_datatype_handle(source.struct_handle)?;
        let position = self
            .target
            .struct_defs
            .iter()
            .position(|definition| definition.struct_handle == handle)
            .with_context(|| {
                format!(
                    "test extension references a struct missing from published module '{}'",
                    self.target.self_id()
                )
            })?;
        self.validate_struct_fields(source.field_information, position)?;
        let mapped = make_index(position, "struct definition", StructDefinitionIndex::new)?;
        self.struct_defs[index.0 as usize] = Some(mapped);
        Ok(mapped)
    }

    fn validate_struct_fields(
        &mut self,
        source: StructFieldInformation,
        target_index: usize,
    ) -> Result<()> {
        let mapped = match source {
            StructFieldInformation::Native => StructFieldInformation::Native,
            StructFieldInformation::Declared(fields) => StructFieldInformation::Declared(
                fields
                    .into_iter()
                    .map(|field| self.map_field_definition(field))
                    .collect::<Result<Vec<_>>>()?,
            ),
        };
        if self.target.struct_defs[target_index].field_information != mapped {
            bail!(
                "struct layout mismatch while linking test extension for '{}'",
                self.target.self_id()
            );
        }
        Ok(())
    }

    fn map_field_definition(&mut self, field: FieldDefinition) -> Result<FieldDefinition> {
        Ok(FieldDefinition {
            name: self.map_identifier(field.name)?,
            signature: move_binary_format::file_format::TypeSignature(
                self.map_signature_token(field.signature.0)?,
            ),
        })
    }

    fn map_enum_definition(&mut self, index: EnumDefinitionIndex) -> Result<EnumDefinitionIndex> {
        if let Some(mapped) = cached(&self.enum_defs, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .enum_defs
            .get(index.0 as usize)
            .with_context(|| format!("invalid source enum definition index {index}"))?
            .clone();
        let handle = self.map_datatype_handle(source.enum_handle)?;
        let position = self
            .target
            .enum_defs
            .iter()
            .position(|definition| definition.enum_handle == handle)
            .with_context(|| {
                format!(
                    "test extension references an enum missing from published module '{}'",
                    self.target.self_id()
                )
            })?;
        let mapped_variants = source
            .variants
            .into_iter()
            .map(|variant| {
                Ok(move_binary_format::file_format::VariantDefinition {
                    variant_name: self.map_identifier(variant.variant_name)?,
                    fields: variant
                        .fields
                        .into_iter()
                        .map(|field| self.map_field_definition(field))
                        .collect::<Result<Vec<_>>>()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if self.target.enum_defs[position].variants != mapped_variants {
            bail!(
                "enum layout mismatch while linking test extension for '{}'",
                self.target.self_id()
            );
        }
        let mapped = make_index(position, "enum definition", EnumDefinitionIndex::new)?;
        self.enum_defs[index.0 as usize] = Some(mapped);
        Ok(mapped)
    }

    fn map_field_handle(&mut self, index: FieldHandleIndex) -> Result<FieldHandleIndex> {
        if let Some(mapped) = cached(&self.field_handles, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .field_handles
            .get(index.0 as usize)
            .with_context(|| format!("invalid source field handle index {index}"))?
            .clone();
        let mapped = FieldHandle {
            owner: self.map_struct_definition(source.owner)?,
            field: source.field,
        };
        let target_index = if let Some(position) = self
            .target
            .field_handles
            .iter()
            .position(|item| item == &mapped)
        {
            make_index(position, "field handle", FieldHandleIndex::new)?
        } else {
            let target_index = make_index(
                self.target.field_handles.len(),
                "field handle",
                FieldHandleIndex::new,
            )?;
            self.target.field_handles.push(mapped);
            target_index
        };
        self.field_handles[index.0 as usize] = Some(target_index);
        Ok(target_index)
    }

    fn map_struct_instantiation(
        &mut self,
        index: StructDefInstantiationIndex,
    ) -> Result<StructDefInstantiationIndex> {
        if let Some(mapped) = cached(&self.struct_instantiations, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .struct_def_instantiations
            .get(index.0 as usize)
            .with_context(|| format!("invalid source struct instantiation index {index}"))?
            .clone();
        let mapped = StructDefInstantiation {
            def: self.map_struct_definition(source.def)?,
            type_parameters: self.map_signature(source.type_parameters)?,
        };
        let target_index = intern(
            &mut self.target.struct_def_instantiations,
            mapped,
            "struct instantiation",
            StructDefInstantiationIndex::new,
        )?;
        self.struct_instantiations[index.0 as usize] = Some(target_index);
        Ok(target_index)
    }

    fn map_function_instantiation(
        &mut self,
        index: FunctionInstantiationIndex,
    ) -> Result<FunctionInstantiationIndex> {
        if let Some(mapped) = cached(&self.function_instantiations, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .function_instantiations
            .get(index.0 as usize)
            .with_context(|| format!("invalid source function instantiation index {index}"))?
            .clone();
        let mapped = FunctionInstantiation {
            handle: self.map_function_handle(source.handle)?,
            type_parameters: self.map_signature(source.type_parameters)?,
        };
        let target_index = intern(
            &mut self.target.function_instantiations,
            mapped,
            "function instantiation",
            FunctionInstantiationIndex::new,
        )?;
        self.function_instantiations[index.0 as usize] = Some(target_index);
        Ok(target_index)
    }

    fn map_field_instantiation(
        &mut self,
        index: FieldInstantiationIndex,
    ) -> Result<FieldInstantiationIndex> {
        if let Some(mapped) = cached(&self.field_instantiations, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .field_instantiations
            .get(index.0 as usize)
            .with_context(|| format!("invalid source field instantiation index {index}"))?
            .clone();
        let mapped = FieldInstantiation {
            handle: self.map_field_handle(source.handle)?,
            type_parameters: self.map_signature(source.type_parameters)?,
        };
        let target_index = intern(
            &mut self.target.field_instantiations,
            mapped,
            "field instantiation",
            FieldInstantiationIndex::new,
        )?;
        self.field_instantiations[index.0 as usize] = Some(target_index);
        Ok(target_index)
    }

    fn map_enum_instantiation(
        &mut self,
        index: EnumDefInstantiationIndex,
    ) -> Result<EnumDefInstantiationIndex> {
        if let Some(mapped) = cached(&self.enum_instantiations, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .enum_def_instantiations
            .get(index.0 as usize)
            .with_context(|| format!("invalid source enum instantiation index {index}"))?
            .clone();
        let mapped = EnumDefInstantiation {
            def: self.map_enum_definition(source.def)?,
            type_parameters: self.map_signature(source.type_parameters)?,
        };
        let target_index = intern(
            &mut self.target.enum_def_instantiations,
            mapped,
            "enum instantiation",
            EnumDefInstantiationIndex::new,
        )?;
        self.enum_instantiations[index.0 as usize] = Some(target_index);
        Ok(target_index)
    }

    fn map_variant_handle(&mut self, index: VariantHandleIndex) -> Result<VariantHandleIndex> {
        if let Some(mapped) = cached(&self.variant_handles, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .variant_handles
            .get(index.0 as usize)
            .with_context(|| format!("invalid source variant handle index {index}"))?
            .clone();
        let mapped = VariantHandle {
            enum_def: self.map_enum_definition(source.enum_def)?,
            variant: source.variant,
        };
        let target_index = intern(
            &mut self.target.variant_handles,
            mapped,
            "variant handle",
            VariantHandleIndex::new,
        )?;
        self.variant_handles[index.0 as usize] = Some(target_index);
        Ok(target_index)
    }

    fn map_variant_instantiation(
        &mut self,
        index: VariantInstantiationHandleIndex,
    ) -> Result<VariantInstantiationHandleIndex> {
        if let Some(mapped) = cached(&self.variant_instantiations, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .variant_instantiation_handles
            .get(index.0 as usize)
            .with_context(|| format!("invalid source variant instantiation index {index}"))?
            .clone();
        let mapped = VariantInstantiationHandle {
            enum_def: self.map_enum_instantiation(source.enum_def)?,
            variant: source.variant,
        };
        let target_index = intern(
            &mut self.target.variant_instantiation_handles,
            mapped,
            "variant instantiation",
            VariantInstantiationHandleIndex::new,
        )?;
        self.variant_instantiations[index.0 as usize] = Some(target_index);
        Ok(target_index)
    }

    fn map_constant(&mut self, index: ConstantPoolIndex) -> Result<ConstantPoolIndex> {
        if let Some(mapped) = cached(&self.constants, index.0)? {
            return Ok(mapped);
        }
        let source = self
            .source
            .constant_pool
            .get(index.0 as usize)
            .with_context(|| format!("invalid source constant index {index}"))?
            .clone();
        let mapped = Constant {
            type_: self.map_signature_token(source.type_)?,
            data: source.data,
        };
        let target_index = intern(
            &mut self.target.constant_pool,
            mapped,
            "constant",
            ConstantPoolIndex::new,
        )?;
        self.constants[index.0 as usize] = Some(target_index);
        Ok(target_index)
    }

    fn map_code_unit(&mut self, code: CodeUnit) -> Result<CodeUnit> {
        Ok(CodeUnit {
            locals: self.map_signature(code.locals)?,
            code: code
                .code
                .into_iter()
                .map(|instruction| self.map_bytecode(instruction))
                .collect::<Result<Vec<_>>>()?,
            jump_tables: code
                .jump_tables
                .into_iter()
                .map(|table| {
                    Ok(VariantJumpTable {
                        head_enum: self.map_enum_definition(table.head_enum)?,
                        jump_table: table.jump_table,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        })
    }

    fn map_bytecode(&mut self, instruction: Bytecode) -> Result<Bytecode> {
        use Bytecode::*;

        Ok(match instruction {
            LdConst(index) => LdConst(self.map_constant(index)?),
            Call(index) => Call(self.map_function_handle(index)?),
            CallGeneric(index) => CallGeneric(self.map_function_instantiation(index)?),
            Pack(index) => Pack(self.map_struct_definition(index)?),
            PackGeneric(index) => PackGeneric(self.map_struct_instantiation(index)?),
            Unpack(index) => Unpack(self.map_struct_definition(index)?),
            UnpackGeneric(index) => UnpackGeneric(self.map_struct_instantiation(index)?),
            MutBorrowField(index) => MutBorrowField(self.map_field_handle(index)?),
            MutBorrowFieldGeneric(index) => {
                MutBorrowFieldGeneric(self.map_field_instantiation(index)?)
            }
            ImmBorrowField(index) => ImmBorrowField(self.map_field_handle(index)?),
            ImmBorrowFieldGeneric(index) => {
                ImmBorrowFieldGeneric(self.map_field_instantiation(index)?)
            }
            VecPack(index, count) => VecPack(self.map_signature(index)?, count),
            VecLen(index) => VecLen(self.map_signature(index)?),
            VecImmBorrow(index) => VecImmBorrow(self.map_signature(index)?),
            VecMutBorrow(index) => VecMutBorrow(self.map_signature(index)?),
            VecPushBack(index) => VecPushBack(self.map_signature(index)?),
            VecPopBack(index) => VecPopBack(self.map_signature(index)?),
            VecUnpack(index, count) => VecUnpack(self.map_signature(index)?, count),
            VecSwap(index) => VecSwap(self.map_signature(index)?),
            PackVariant(index) => PackVariant(self.map_variant_handle(index)?),
            PackVariantGeneric(index) => PackVariantGeneric(self.map_variant_instantiation(index)?),
            UnpackVariant(index) => UnpackVariant(self.map_variant_handle(index)?),
            UnpackVariantImmRef(index) => UnpackVariantImmRef(self.map_variant_handle(index)?),
            UnpackVariantMutRef(index) => UnpackVariantMutRef(self.map_variant_handle(index)?),
            UnpackVariantGeneric(index) => {
                UnpackVariantGeneric(self.map_variant_instantiation(index)?)
            }
            UnpackVariantGenericImmRef(index) => {
                UnpackVariantGenericImmRef(self.map_variant_instantiation(index)?)
            }
            UnpackVariantGenericMutRef(index) => {
                UnpackVariantGenericMutRef(self.map_variant_instantiation(index)?)
            }
            ExistsDeprecated(index) => ExistsDeprecated(self.map_struct_definition(index)?),
            ExistsGenericDeprecated(index) => {
                ExistsGenericDeprecated(self.map_struct_instantiation(index)?)
            }
            MoveFromDeprecated(index) => MoveFromDeprecated(self.map_struct_definition(index)?),
            MoveFromGenericDeprecated(index) => {
                MoveFromGenericDeprecated(self.map_struct_instantiation(index)?)
            }
            MoveToDeprecated(index) => MoveToDeprecated(self.map_struct_definition(index)?),
            MoveToGenericDeprecated(index) => {
                MoveToGenericDeprecated(self.map_struct_instantiation(index)?)
            }
            MutBorrowGlobalDeprecated(index) => {
                MutBorrowGlobalDeprecated(self.map_struct_definition(index)?)
            }
            MutBorrowGlobalGenericDeprecated(index) => {
                MutBorrowGlobalGenericDeprecated(self.map_struct_instantiation(index)?)
            }
            ImmBorrowGlobalDeprecated(index) => {
                ImmBorrowGlobalDeprecated(self.map_struct_definition(index)?)
            }
            ImmBorrowGlobalGenericDeprecated(index) => {
                ImmBorrowGlobalGenericDeprecated(self.map_struct_instantiation(index)?)
            }
            Pop | Ret | BrTrue(_) | BrFalse(_) | Branch(_) | LdU8(_) | LdU16(_) | LdU32(_)
            | LdU64(_) | LdU128(_) | LdU256(_) | CastU8 | CastU16 | CastU32 | CastU64
            | CastU128 | CastU256 | LdTrue | LdFalse | CopyLoc(_) | MoveLoc(_) | StLoc(_)
            | ReadRef | WriteRef | FreezeRef | MutBorrowLoc(_) | ImmBorrowLoc(_) | Add | Sub
            | Mul | Mod | Div | BitOr | BitAnd | Xor | Or | And | Not | Eq | Neq | Lt | Gt | Le
            | Ge | Abort | Nop | Shl | Shr | VariantSwitch(_) => instruction,
        })
    }
}

fn local_function_definitions(
    module: &CompiledModule,
) -> Result<BTreeMap<String, &FunctionDefinition>> {
    module
        .function_defs
        .iter()
        .map(|definition| {
            let handle = module
                .function_handles
                .get(definition.function.0 as usize)
                .with_context(|| {
                    format!(
                        "module '{}' contains invalid function definition handle {}",
                        module.self_id(),
                        definition.function
                    )
                })?;
            let name = module
                .identifiers
                .get(handle.name.0 as usize)
                .with_context(|| {
                    format!(
                        "module '{}' contains invalid function name index {}",
                        module.self_id(),
                        handle.name
                    )
                })?
                .to_string();
            Ok((name, definition))
        })
        .collect()
}

fn cached<Index: Copy>(cache: &[Option<Index>], index: TableIndex) -> Result<Option<Index>> {
    cache
        .get(index as usize)
        .copied()
        .with_context(|| format!("invalid source table index {index}"))
}

fn intern<T: PartialEq, Index>(
    table: &mut Vec<T>,
    value: T,
    table_name: &str,
    constructor: impl FnOnce(TableIndex) -> Index + Copy,
) -> Result<Index> {
    if let Some(position) = table.iter().position(|item| item == &value) {
        return make_index(position, table_name, constructor);
    }
    let index = make_index(table.len(), table_name, constructor)?;
    table.push(value);
    Ok(index)
}

fn make_index<Index>(
    position: usize,
    table_name: &str,
    constructor: impl FnOnce(TableIndex) -> Index,
) -> Result<Index> {
    let position = TableIndex::try_from(position)
        .with_context(|| format!("{table_name} table exceeds the Move bytecode index limit"))?;
    Ok(constructor(position))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        move_ir_to_bytecode::{compiler::compile_module, parser::parse_module},
    };

    const BASE_MODULE: &str = r#"
        mvir 0x42::Hidden {
            public struct Secret has drop { value: u64 }

            public fun real(x: u64): u64 {
            label b0:
                return move(x) + 1;
            }

            public fun make(x: u64): Self::Secret {
            label b0:
                return Secret { value: move(x) };
            }
        }
    "#;

    const INTERFACE_WITH_EXTENSION: &str = r#"
        mvir 0x42::Hidden {
            public struct Secret has drop { value: u64 }

            native public fun real(x: u64): u64;
            native public fun make(x: u64): Self::Secret;

            public fun fixture_and_execute(x: u64): u64 {
                let secret: Self::Secret;
                let secret_ref: &Self::Secret;
                let value: u64;
            label b0:
                secret = Self::make(move(x));
                secret_ref = &secret;
                value = *&move(secret_ref).Secret::value;
                _ = move(secret);
                return Self::real(move(value));
            }
        }
    "#;

    fn compile(source: &str) -> CompiledModule {
        compile_module(parse_module(source).expect("valid Move IR"), [])
            .expect("valid compiled module")
            .0
    }

    #[test]
    fn preserves_published_functions_and_adds_the_selected_fixture() {
        let published = compile(BASE_MODULE);
        let interface = compile(INTERFACE_WITH_EXTENSION);
        let published_function_count = published.function_defs.len();
        let linked = add_test_functions(
            published.clone(),
            &interface,
            &BTreeSet::from(["fixture_and_execute".to_owned()]),
        )
        .expect("fixture links");

        assert!(!linked.publishable);
        assert_eq!(
            linked.self_module_handle_idx,
            published.self_module_handle_idx
        );
        assert_eq!(linked.friend_decls, published.friend_decls);
        assert_eq!(linked.metadata, published.metadata);
        macro_rules! assert_published_prefix {
            ($field:ident) => {
                assert_eq!(
                    &linked.$field[..published.$field.len()],
                    published.$field.as_slice(),
                    "published {} changed",
                    stringify!($field),
                );
            };
        }
        assert_published_prefix!(module_handles);
        assert_published_prefix!(datatype_handles);
        assert_published_prefix!(function_handles);
        assert_published_prefix!(field_handles);
        assert_published_prefix!(struct_def_instantiations);
        assert_published_prefix!(function_instantiations);
        assert_published_prefix!(field_instantiations);
        assert_published_prefix!(signatures);
        assert_published_prefix!(identifiers);
        assert_published_prefix!(address_identifiers);
        assert_published_prefix!(constant_pool);
        assert_published_prefix!(struct_defs);
        assert_published_prefix!(enum_defs);
        assert_published_prefix!(enum_def_instantiations);
        assert_published_prefix!(variant_handles);
        assert_published_prefix!(variant_instantiation_handles);
        assert_eq!(
            &linked.function_defs[..published_function_count],
            published.function_defs.as_slice()
        );
        assert_eq!(linked.function_defs.len(), published_function_count + 1);

        let extension = linked.function_defs.last().expect("extension definition");
        let code = &extension.code.as_ref().expect("extension body").code;
        let published_handles = local_function_definitions(&published)
            .expect("valid published functions")
            .into_values()
            .map(|definition| definition.function)
            .collect::<BTreeSet<_>>();
        assert!(code.iter().any(|instruction| {
            matches!(instruction, Bytecode::Call(handle) if published_handles.contains(handle))
        }));
        assert!(code
            .iter()
            .any(|instruction| matches!(instruction, Bytecode::ImmBorrowField(_))));
    }

    #[test]
    fn rejects_a_fixture_that_redefines_a_published_function() {
        let published = compile(BASE_MODULE);
        let interface = compile(INTERFACE_WITH_EXTENSION);
        let error = add_test_functions(published, &interface, &BTreeSet::from(["real".to_owned()]))
            .expect_err("published definitions cannot be replaced");

        assert!(error
            .to_string()
            .contains("cannot redefine published function"));
    }

    #[test]
    fn rejects_a_stale_interface_signature() {
        let published = compile(BASE_MODULE);
        let interface = compile(
            r#"
                mvir 0x42::Hidden {
                    public struct Secret has drop { value: u64 }
                    native public fun real(x: bool): u64;
                    native public fun make(x: u64): Self::Secret;

                    public fun fixture_and_execute(x: bool): u64 {
                    label b0:
                        return Self::real(move(x));
                    }
                }
            "#,
        );
        let error = add_test_functions(
            published,
            &interface,
            &BTreeSet::from(["fixture_and_execute".to_owned()]),
        )
        .expect_err("a stale interface must fail");

        assert!(error.to_string().contains("function ABI mismatch"));
    }
}
