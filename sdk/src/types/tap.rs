//! Standard TAP models shared by SDK, CLI, leader, and future Move surfaces.

use {
    crate::{
        move_bindings::{
            interface::{
                agent::{SkillDagBinding, SkillRequirement},
                payment::{PaymentSourceKind, SkillPaymentPolicy},
                version::InterfaceVersion,
            },
            registry::agent_registry::{AgentRecord, DefaultDagExecutor, SkillRecord},
        },
        sui,
    },
    serde::{Deserialize, Serialize},
    sha2::{Digest as _, Sha256},
    std::{fmt, path::PathBuf},
};

/// On-chain generated standard Talus agent ID.
pub type AgentId = sui::types::Address;

/// Agent-local standard TAP skill identity index.
pub type SkillId = u64;

pub const fn skill_id(value: u64) -> SkillId {
    value
}

/// Key for a pinned skill interface revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillRevisionLookupKey {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceVersion,
}

impl fmt::Display for SkillRevisionLookupKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.agent_id, self.skill_id, self.interface_revision
        )
    }
}

/// Key for fresh worksheet and active-revision lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorksheetKey {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
}

impl fmt::Display for WorksheetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.agent_id, self.skill_id)
    }
}

/// Fetched skill record plus dynamic-table keys that are not stored in Move.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRecordContext {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub record: SkillRecord,
}

impl SkillRecordContext {
    pub fn active(&self) -> bool {
        self.record.active
    }

    pub fn dag_binding(&self) -> &SkillDagBinding {
        &self.record.dag_binding
    }

    pub fn current_interface_revision(&self) -> InterfaceVersion {
        self.record.current_interface_revision
    }

    pub fn requirements(&self) -> &SkillRequirement {
        &self.record.requirements
    }
}

/// SDK-resolved default DAG executor target for arbitrary-DAG execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DefaultDagExecutorTarget {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
}

/// Expanded `nexus_registry::agent_registry::AgentRegistry` contents with table entries fetched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRegistrySnapshot {
    pub id: sui::types::Address,
    pub agents: Vec<AgentRecord>,
    pub skills: Vec<SkillRecordContext>,
    #[serde(default)]
    pub default_executor: Option<DefaultDagExecutor>,
}

impl AgentRegistrySnapshot {
    /// Convert current skill revisions into leader-facing skill revision records.
    pub fn skill_revision_records(&self) -> anyhow::Result<Vec<SkillRevisionContext>> {
        self.skills
            .iter()
            .filter_map(SkillRevisionContext::from_skill_record)
            .map(|record| {
                record.validate()?;
                Ok(record)
            })
            .collect()
    }

    pub fn skill_revision_record(
        &self,
        key: SkillRevisionLookupKey,
    ) -> anyhow::Result<SkillRevisionContext> {
        let matches = self
            .skill_revision_records()?
            .into_iter()
            .filter(|skill_revision| skill_revision.key == key)
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [] => anyhow::bail!("TAP skill revision not found for key {key}"),
            [skill_revision] => Ok(skill_revision.clone()),
            _ => anyhow::bail!("duplicate TAP skill revisions found for key {key}"),
        }
    }

    pub fn active_skill_revision_record(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
    ) -> anyhow::Result<SkillRevisionContext> {
        let skills = self
            .skills
            .iter()
            .filter(|skill| {
                skill.agent_id == agent_id && skill.skill_id == skill_id && skill.active()
            })
            .collect::<Vec<_>>();

        let skill = match skills.as_slice() {
            [] => {
                return Err(
                    SkillRevisionLookupError::MissingActiveRevision { agent_id, skill_id }.into(),
                )
            }
            [skill] if skill.active() => *skill,
            [_] => {
                return Err(
                    SkillRevisionLookupError::MissingActiveRevision { agent_id, skill_id }.into(),
                )
            }
            _ => {
                return Err(SkillRevisionLookupError::DuplicateActiveRevision {
                    agent_id,
                    skill_id,
                    count: skills.len(),
                }
                .into())
            }
        };

        let skill_revision = SkillRevisionContext {
            key: SkillRevisionLookupKey {
                agent_id,
                skill_id,
                interface_revision: skill.current_interface_revision(),
            },
            requirements: skill.requirements().clone(),
        };
        skill_revision.validate()?;
        Ok(skill_revision)
    }

    pub fn default_dag_executor(&self) -> anyhow::Result<&DefaultDagExecutor> {
        self.default_executor
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("AgentRegistry missing default agent"))
    }
}

/// Active or pinned skill revision record returned to leader and SDK callers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRevisionContext {
    pub key: SkillRevisionLookupKey,
    pub requirements: SkillRequirement,
}

impl SkillRevisionContext {
    fn from_skill_record(skill: &SkillRecordContext) -> Option<Self> {
        Some(Self {
            key: SkillRevisionLookupKey {
                agent_id: skill.agent_id,
                skill_id: skill.skill_id,
                interface_revision: skill.current_interface_revision(),
            },
            requirements: skill.requirements().clone(),
        })
    }

    pub fn worksheet_key(&self) -> WorksheetKey {
        WorksheetKey {
            agent_id: self.key.agent_id,
            skill_id: self.key.skill_id,
        }
    }

    pub fn validate(&self) -> Result<(), TapValidationError> {
        validate_requirements(&self.requirements)?;

        Ok(())
    }
}

/// Registered skill plus the currently active skill revision used for fresh standard execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveSkillExecutionTarget {
    pub skill: SkillRecordContext,
    pub skill_revision: SkillRevisionContext,
}

/// Default execution target plus active skill revision recovered for fresh default DAG execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultDagExecutorRecord {
    pub target: DefaultDagExecutorTarget,
    pub skill: SkillRecordContext,
    pub skill_revision: SkillRevisionContext,
}

/// DAG-backed TAP skill config used by SDK/CLI authoring helpers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillConfig {
    pub name: String,
    pub dag_path: PathBuf,
    pub requirements: SkillRequirement,
    pub interface_revision: InterfaceVersion,
}

impl SkillConfig {
    pub fn validate(&self) -> Result<(), TapValidationError> {
        if self.name.trim().is_empty() {
            return Err(TapValidationError::MissingSkillName);
        }
        if self.dag_path.as_os_str().is_empty() {
            return Err(TapValidationError::MissingDagPath);
        }

        validate_requirements(&self.requirements)
    }
}

/// Published TAP plus DAG artifact used when binding an agent skill.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapPublishArtifact {
    pub skill_name: String,
    pub dag_id: sui::types::Address,
    pub interface_revision: InterfaceVersion,
    pub requirements: SkillRequirement,
}

impl TapPublishArtifact {
    pub fn from_config(config: &SkillConfig, dag_id: sui::types::Address) -> anyhow::Result<Self> {
        config.validate()?;

        Ok(Self {
            skill_name: config.name.clone(),
            dag_id,
            interface_revision: config.interface_revision,
            requirements: config.requirements.clone(),
        })
    }
}

pub fn tap_input_commitment_from_dag_inputs<I, V, P>(inputs: I) -> Vec<u8>
where
    I: IntoIterator<Item = (V, P)>,
    V: AsRef<str>,
    P: AsRef<str>,
{
    let mut canonical_inputs = inputs
        .into_iter()
        .map(|(vertex, port)| (vertex.as_ref().to_string(), port.as_ref().to_string()))
        .collect::<Vec<_>>();
    canonical_inputs.sort();

    let encoded =
        bcs::to_bytes(&canonical_inputs).expect("canonical TAP DAG input pairs should encode");
    Sha256::digest(encoded).to_vec()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TapValidationError {
    MissingSkillName,
    MissingDagPath,
    MissingInputCommitment,
    EmptyAuthorizedToolModule,
    EmptyAuthorizedToolFunction,
}

impl fmt::Display for TapValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TapValidationError::MissingSkillName => write!(f, "skill name is required"),
            TapValidationError::MissingDagPath => write!(f, "DAG path is required"),
            TapValidationError::MissingInputCommitment => write!(f, "input commitment is required"),
            TapValidationError::EmptyAuthorizedToolModule => {
                write!(f, "authorized tool module is required")
            }
            TapValidationError::EmptyAuthorizedToolFunction => {
                write!(f, "authorized tool function is required")
            }
        }
    }
}

impl std::error::Error for TapValidationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkillRevisionLookupError {
    MissingActiveRevision {
        agent_id: AgentId,
        skill_id: SkillId,
    },
    DuplicateActiveRevision {
        agent_id: AgentId,
        skill_id: SkillId,
        count: usize,
    },
    InvalidSkillRevision(TapValidationError),
}

impl fmt::Display for SkillRevisionLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillRevisionLookupError::MissingActiveRevision { agent_id, skill_id } => {
                write!(f, "no active TAP skill revision for agent_id={agent_id}, skill_id={skill_id}")
            }
            SkillRevisionLookupError::DuplicateActiveRevision {
                agent_id,
                skill_id,
                count,
            } => write!(
                f,
                "expected one active TAP skill revision for agent_id={agent_id}, skill_id={skill_id}, found {count}"
            ),
            SkillRevisionLookupError::InvalidSkillRevision(error) => {
                write!(f, "invalid TAP skill revision: {error}")
            }
        }
    }
}

impl std::error::Error for SkillRevisionLookupError {}

pub fn validate_requirements(requirements: &SkillRequirement) -> Result<(), TapValidationError> {
    if requirements.input_commitment.is_empty() {
        return Err(TapValidationError::MissingInputCommitment);
    }
    for tool in &requirements.fixed_tools {
        if tool.tool_fqn_string().trim().is_empty() {
            return Err(TapValidationError::EmptyAuthorizedToolModule);
        }
    }

    Ok(())
}

pub fn validate_execution_payment_options(
    agent_id: AgentId,
    policy: &SkillPaymentPolicy,
    payment_source: &[u8],
    payment_max_budget: u64,
    payer: sui::types::Address,
) -> anyhow::Result<()> {
    match policy {
        SkillPaymentPolicy::UserFunded => {
            let expected = bcs::to_bytes(&PaymentSourceKind::user_funded(payer))?;
            let source_is_valid =
                payment_source.is_empty() || payment_source == expected.as_slice();
            if !source_is_valid {
                anyhow::bail!(
                    "standard TAP user-funded payment source must be empty or generated user-funded source BCS"
                );
            }
        }
        SkillPaymentPolicy::AgentFunded { max_budget } => {
            if payment_max_budget == 0 || payment_max_budget > *max_budget {
                anyhow::bail!(
                    "standard TAP agent-funded payment budget {} must be positive and no greater than skill policy max {}",
                    payment_max_budget,
                    max_budget
                );
            }
            let expected = bcs::to_bytes(&PaymentSourceKind::agent_funded(agent_id))?;
            let source_is_valid = payment_source == expected.as_slice();
            if !source_is_valid {
                anyhow::bail!(
                    "standard Talus agent-funded payment source must be generated agent-funded source BCS"
                );
            }
        }
    }

    Ok(())
}

pub fn payment_source_from_address(address: sui::types::Address) -> anyhow::Result<Vec<u8>> {
    Ok(bcs::to_bytes(&PaymentSourceKind::user_funded(address))?)
}

/// Resolve exactly one active skill revision for fresh execution.
pub fn resolve_active_tap_skill_revision<'a>(
    records: &'a [SkillRevisionContext],
    skills: &[SkillRecordContext],
    agent_id: AgentId,
    skill_id: SkillId,
) -> Result<&'a SkillRevisionContext, SkillRevisionLookupError> {
    let skill_matches = skills
        .iter()
        .filter(|skill| skill.agent_id == agent_id && skill.skill_id == skill_id && skill.active())
        .collect::<Vec<_>>();

    let skill = match skill_matches.as_slice() {
        [] => return Err(SkillRevisionLookupError::MissingActiveRevision { agent_id, skill_id }),
        [skill] if skill.active() => *skill,
        [_] => return Err(SkillRevisionLookupError::MissingActiveRevision { agent_id, skill_id }),
        _ => {
            return Err(SkillRevisionLookupError::DuplicateActiveRevision {
                agent_id,
                skill_id,
                count: skill_matches.len(),
            })
        }
    };

    let active = records
        .iter()
        .filter(|record| {
            record.key.agent_id == agent_id
                && record.key.skill_id == skill_id
                && record.key.interface_revision == skill.current_interface_revision()
        })
        .collect::<Vec<_>>();

    match active.as_slice() {
        [] => Err(SkillRevisionLookupError::MissingActiveRevision { agent_id, skill_id }),
        [record] => {
            record
                .validate()
                .map_err(SkillRevisionLookupError::InvalidSkillRevision)?;
            Ok(record)
        }
        _ => Err(SkillRevisionLookupError::DuplicateActiveRevision {
            agent_id,
            skill_id,
            count: active.len(),
        }),
    }
}

/// Resolve the unique active skill and skill revision for fresh standard execution.
pub fn resolve_active_tap_skill_execution_target(
    registry: &AgentRegistrySnapshot,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<ActiveSkillExecutionTarget> {
    let skill_matches = registry
        .skills
        .iter()
        .filter(|skill| skill.agent_id == agent_id && skill.skill_id == skill_id && skill.active())
        .collect::<Vec<_>>();

    let skill = match skill_matches.as_slice() {
        [] => anyhow::bail!(
            "active TAP skill not found for agent {} skill {}",
            agent_id,
            skill_id
        ),
        [skill] => (*skill).clone(),
        _ => anyhow::bail!(
            "duplicate active TAP skills found for agent {} skill {}",
            agent_id,
            skill_id
        ),
    };

    let skill_revision = registry.active_skill_revision_record(agent_id, skill_id)?;

    Ok(ActiveSkillExecutionTarget {
        skill,
        skill_revision,
    })
}

/// Resolve the configured default agent from registry state.
pub fn resolve_default_tap_dag_executor(
    registry: &AgentRegistrySnapshot,
) -> anyhow::Result<DefaultDagExecutorRecord> {
    let default_executor = registry.default_dag_executor()?;
    let target = default_executor.target();
    let execution_target =
        resolve_active_tap_skill_execution_target(registry, target.agent_id, target.skill_id)?;

    if execution_target.skill.dag_binding() != &SkillDagBinding::RuntimeSelected {
        anyhow::bail!(
            "default agent skill {} for agent {} is not runtime-DAG selected",
            target.skill_id,
            target.agent_id
        );
    }

    Ok(DefaultDagExecutorRecord {
        target,
        skill: execution_target.skill,
        skill_revision: execution_target.skill_revision,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::move_bindings::{
            interface::{
                agent::{Agent, SkillSchedulePolicy},
                payment::{
                    ExecutionPaymentFinalState, ScheduledOccurrenceFinalState,
                    VertexExecutionPaymentSettlementKind,
                },
            },
            registry::agent_registry::AgentRegistry,
            sui_framework::table::Table as MoveTable,
        },
        std::str::FromStr,
    };

    fn addr(value: &str) -> sui::types::Address {
        sui::types::Address::from_str(value).expect("valid address")
    }

    fn requirements() -> SkillRequirement {
        SkillRequirement {
            input_commitment: vec![1],
            payment_policy: SkillPaymentPolicy::AgentFunded { max_budget: 100 },
            schedule_policy: SkillSchedulePolicy::default(),
            fixed_tools: Vec::new(),
        }
    }

    fn skill_revision(revision: u64) -> SkillRevisionContext {
        SkillRevisionContext {
            key: SkillRevisionLookupKey {
                agent_id: addr("0xa"),
                skill_id: 11,
                interface_revision: InterfaceVersion::new(revision),
            },
            requirements: requirements(),
        }
    }

    fn skill(active: bool, current_interface_revision: u64) -> SkillRecordContext {
        SkillRecordContext {
            agent_id: addr("0xa"),
            skill_id: 11,
            record: SkillRecord {
                description: vec![3],
                active,
                dag_binding: SkillDagBinding::pinned(addr("0x44")),
                requirements: requirements(),
                current_interface_revision: InterfaceVersion::new(current_interface_revision),
                scheduled_task_count: 0,
            },
        }
    }

    fn registry_with_active_skill() -> AgentRegistrySnapshot {
        AgentRegistrySnapshot {
            id: addr("0xf"),
            agents: Vec::new(),
            skills: vec![skill(true, 2)],
            default_executor: None,
        }
    }

    #[test]
    fn skill_revision_records_are_derived_from_current_skills() {
        let registry = registry_with_active_skill();
        let records = registry.skill_revision_records().expect("derived records");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].key.agent_id, addr("0xa"));
        assert_eq!(records[0].key.skill_id, 11);
        assert_eq!(records[0].key.interface_revision, InterfaceVersion::new(2));
    }

    #[test]
    fn validate_rejects_missing_input_commitment() {
        let mut requirements = requirements();
        requirements.input_commitment.clear();

        assert_eq!(
            validate_requirements(&requirements),
            Err(TapValidationError::MissingInputCommitment)
        );
    }

    #[test]
    fn active_resolution_requires_exactly_one_active_revision() {
        let active = skill_revision(1);
        let inactive = skill_revision(2);
        let records = vec![active.clone(), inactive];
        let skills = vec![skill(true, 1)];

        let resolved = resolve_active_tap_skill_revision(
            &records,
            &skills,
            active.key.agent_id,
            active.key.skill_id,
        )
        .expect("one active skill revision");

        assert_eq!(resolved.key.interface_revision, InterfaceVersion::new(1));

        let duplicate = vec![skill_revision(1), skill_revision(1)];
        assert!(matches!(
            resolve_active_tap_skill_revision(&duplicate, &skills, addr("0xa"), 11),
            Err(SkillRevisionLookupError::DuplicateActiveRevision { count: 2, .. })
        ));
    }

    #[cfg(feature = "bcs")]
    #[test]
    fn agent_registry_object_bcs_decodes_without_inline_default_executor() {
        let raw = AgentRegistry {
            id: crate::move_bindings::sui_framework::object::UID::new(addr("0xf")),
            agents: MoveTable::new(addr("0x90"), 0),
        };
        let bytes = bcs::to_bytes(&raw).expect("raw Move registry BCS should encode");
        let decoded: AgentRegistry =
            bcs::from_bytes(&bytes).expect("raw Move registry BCS should decode");

        assert_eq!(sui::types::Address::from(decoded.id), addr("0xf"));
        assert_eq!(decoded.agents.id(), addr("0x90"));
    }

    #[test]
    fn publish_artifact_contains_skill_artifact_fields() {
        let config = SkillConfig {
            name: "weather".to_string(),
            dag_path: PathBuf::from("skill.dag.json"),
            requirements: requirements(),
            interface_revision: InterfaceVersion::new(1),
        };

        let artifact =
            TapPublishArtifact::from_config(&config, addr("0x8")).expect("valid artifact");

        assert_eq!(artifact.dag_id, addr("0x8"));
        assert_eq!(artifact.skill_name, "weather");
    }

    #[test]
    fn publish_artifact_preserves_current_skill_inputs() {
        let config = SkillConfig {
            name: "weather".to_string(),
            dag_path: PathBuf::from("skill.dag.json"),
            requirements: requirements(),
            interface_revision: InterfaceVersion::new(1),
        };
        let artifact =
            TapPublishArtifact::from_config(&config, addr("0x8")).expect("valid artifact");

        assert_eq!(artifact.interface_revision, InterfaceVersion::new(1));
        assert_eq!(artifact.requirements, config.requirements);
    }

    #[test]
    fn dag_input_commitment_is_order_independent() {
        let first = tap_input_commitment_from_dag_inputs([
            ("weather::V2", "city"),
            ("weather::V1", "country"),
        ]);
        let second = tap_input_commitment_from_dag_inputs([
            ("weather::V1", "country"),
            ("weather::V2", "city"),
        ]);

        assert_eq!(first, second);
        assert_eq!(first.len(), 32);
    }

    #[test]
    fn validate_execution_payment_options_enforces_user_funded_policy() {
        let agent = addr("0xa");
        let payer = addr("0x1");
        let explicit_source = payment_source_from_address(payer).expect("payer source");
        let other_source = payment_source_from_address(addr("0x2")).expect("other source");
        let policy = SkillPaymentPolicy::UserFunded;

        validate_execution_payment_options(agent, &policy, &[], 100, payer)
            .expect("implicit payer source");
        validate_execution_payment_options(agent, &policy, &explicit_source, 100, payer)
            .expect("explicit payer source");
        assert!(
            validate_execution_payment_options(agent, &policy, &other_source, 100, payer,).is_err()
        );
    }

    #[test]
    fn validate_execution_payment_options_enforces_source_modes() {
        let agent = addr("0xa");
        let payer = addr("0x1");
        let user_funded_agent_source = payment_source_from_address(agent).expect("agent source");
        let agent_source =
            bcs::to_bytes(&PaymentSourceKind::agent_funded(agent)).expect("agent vault source");

        let agent_funded = SkillPaymentPolicy::AgentFunded { max_budget: 100 };
        validate_execution_payment_options(agent, &agent_funded, &agent_source, 100, payer)
            .expect("agent-funded source at policy cap");
        assert!(
            validate_execution_payment_options(agent, &agent_funded, &[], 100, payer,).is_err()
        );
        assert!(validate_execution_payment_options(
            agent,
            &agent_funded,
            &user_funded_agent_source,
            100,
            payer,
        )
        .is_err());
        assert!(validate_execution_payment_options(
            agent,
            &agent_funded,
            &agent_source,
            101,
            payer,
        )
        .is_err());
    }

    #[test]
    fn tap_generated_enums_bcs_roundtrip() {
        for value in [
            PaymentSourceKind::user_funded(addr("0x1")),
            PaymentSourceKind::agent_funded(addr("0xa")),
        ] {
            assert_eq!(
                bcs::from_bytes::<PaymentSourceKind>(&bcs::to_bytes(&value).unwrap()).unwrap(),
                value
            );
        }

        for value in [
            VertexExecutionPaymentSettlementKind::Paid,
            VertexExecutionPaymentSettlementKind::Ticket,
        ] {
            assert_eq!(
                bcs::from_bytes::<VertexExecutionPaymentSettlementKind>(
                    &bcs::to_bytes(&value).unwrap()
                )
                .unwrap(),
                value
            );
        }

        let payment_state = ExecutionPaymentFinalState::Accomplished;
        assert_eq!(
            bcs::from_bytes::<ExecutionPaymentFinalState>(&bcs::to_bytes(&payment_state).unwrap())
                .unwrap(),
            payment_state
        );

        let occurrence_state = ScheduledOccurrenceFinalState::InFlight;
        assert_eq!(
            bcs::from_bytes::<ScheduledOccurrenceFinalState>(
                &bcs::to_bytes(&occurrence_state).unwrap()
            )
            .unwrap(),
            occurrence_state
        );
    }

    #[test]
    fn tap_authorization_template_bcs_roundtrip() {
        let template =
            crate::move_bindings::interface::authorization::AgentVertexAuthorizationTemplate {
                skill_id: 7,
                vertex: crate::move_bindings::move_std::ascii::String::from("0xnothex"),
                recipient_id: crate::move_bindings::sui_framework::object::ID::new(addr("0xda6")),
            };
        let decoded: crate::move_bindings::interface::authorization::AgentVertexAuthorizationTemplate =
            bcs::from_bytes(&bcs::to_bytes(&template).unwrap()).unwrap();
        assert_eq!(decoded, template);
        assert_eq!(template.vertex.as_str(), "0xnothex");
    }

    #[test]
    fn tap_payment_source_bcs_roundtrips_and_rejects_unknown_kind() {
        let invoker = addr("0x21");
        let typed = bcs::from_bytes::<PaymentSourceKind>(
            &payment_source_from_address(invoker).expect("typed invoker source"),
        )
        .expect("typed invoker source decodes");
        assert_eq!(typed, PaymentSourceKind::user_funded(invoker));
        assert_eq!(typed.identity(), invoker);

        let agent = addr("0x22");
        let typed = bcs::from_bytes::<PaymentSourceKind>(
            &bcs::to_bytes(&PaymentSourceKind::agent_funded(agent)).expect("typed vault source"),
        )
        .expect("typed vault source decodes");
        assert_eq!(typed, PaymentSourceKind::agent_funded(agent));
        assert_eq!(typed.identity(), agent);

        assert!(bcs::from_bytes::<PaymentSourceKind>(&[9]).is_err());
    }

    #[test]
    fn active_skill_execution_target_requires_one_active_skill_and_endpoint() {
        let registry = registry_with_active_skill();
        let resolved = resolve_active_tap_skill_execution_target(&registry, addr("0xa"), 11)
            .expect("active skill target");

        assert_eq!(
            *resolved.skill.dag_binding(),
            SkillDagBinding::pinned(addr("0x44"))
        );
        assert_eq!(
            resolved.skill_revision.key.interface_revision,
            InterfaceVersion::new(2)
        );
    }

    #[test]
    fn default_dag_executor_requires_runtime_selected_skill() {
        let mut registry = registry_with_active_skill();
        registry.default_executor = Some(DefaultDagExecutor {
            agent: Agent::from_ids(addr("0xa"), 1, Some(registry.id)),
            skill_id: 11,
        });

        let error = resolve_default_tap_dag_executor(&registry)
            .expect_err("pinned skill cannot be default runtime target");
        assert!(error.to_string().contains("is not runtime-DAG selected"));

        registry.skills[0].record.dag_binding = SkillDagBinding::runtime_selected();
        let target = resolve_default_tap_dag_executor(&registry)
            .expect("runtime-selected default skill resolves");

        assert_eq!(
            target.target,
            DefaultDagExecutorTarget {
                agent_id: addr("0xa"),
                skill_id: 11,
            }
        );
        assert_eq!(
            target.skill.dag_binding(),
            &SkillDagBinding::RuntimeSelected
        );
    }

    #[test]
    fn active_skill_execution_target_rejects_missing_skill() {
        let mut registry = registry_with_active_skill();
        registry.skills.clear();

        assert!(resolve_active_tap_skill_execution_target(&registry, addr("0xa"), 11).is_err());
    }
}
