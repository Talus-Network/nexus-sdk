//! Limits Walrus Move calls to a separate coin containing the approved budget.

use {
    crate::sui::types::{
        Address,
        Argument,
        Command,
        Input,
        ProgrammableTransaction,
        SplitCoins,
        TransferObjects,
    },
    anyhow::{bail, ensure, Context as _},
};

pub(super) fn cap_payment(
    ptb: &mut ProgrammableTransaction,
    owner: Address,
    budget: u64,
) -> anyhow::Result<()> {
    let first = ptb
        .commands
        .iter()
        .position(is_payment)
        .context("Walrus transaction has no storage payment")?;
    let Command::MoveCall(call) = &ptb.commands[first] else {
        unreachable!()
    };
    let coin = *call
        .arguments
        .last()
        .context("Walrus payment has no coin argument")?;
    ensure!(
        matches!(coin, Argument::Input(_)),
        "unexpected Walrus payment coin source"
    );
    let amount = Argument::Input(ptb.inputs.len().try_into()?);
    ptb.inputs.push(Input::Pure(bcs::to_bytes(&budget)?));
    let recipient = Argument::Input(ptb.inputs.len().try_into()?);
    ptb.inputs.push(Input::Pure(bcs::to_bytes(&owner)?));
    let first: u16 = first.try_into()?;
    let limited = Argument::NestedResult(first, 0);
    for command in &mut ptb.commands {
        remap(command, first)?;
        if is_payment(command) {
            let Command::MoveCall(call) = command else {
                unreachable!()
            };
            let payment = call
                .arguments
                .last_mut()
                .context("Walrus payment has no coin")?;
            ensure!(
                *payment == coin,
                "Walrus transaction uses more than one payment coin"
            );
            *payment = limited;
        }
    }
    ptb.commands.insert(
        first as usize,
        Command::SplitCoins(SplitCoins {
            coin,
            amounts: vec![amount],
        }),
    );
    ptb.commands.push(Command::TransferObjects(TransferObjects {
        objects: vec![limited],
        address: recipient,
    }));
    Ok(())
}

fn is_payment(command: &Command) -> bool {
    matches!(command, Command::MoveCall(call) if call.module.as_str() == "system" &&
        matches!(call.function.as_str(), "reserve_space" | "register_blob" | "extend_blob"))
}

fn remap(command: &mut Command, first: u16) -> anyhow::Result<()> {
    let shift = |argument: &mut Argument| -> anyhow::Result<()> {
        let index = match argument {
            Argument::Result(index) | Argument::NestedResult(index, _) => index,
            _ => return Ok(()),
        };
        if *index >= first {
            *index = index
                .checked_add(1)
                .context("Walrus command index overflow")?;
        }
        Ok(())
    };
    match command {
        Command::MoveCall(call) => {
            for arg in &mut call.arguments {
                shift(arg)?;
            }
        }
        Command::TransferObjects(call) => {
            shift(&mut call.address)?;
            for arg in &mut call.objects {
                shift(arg)?;
            }
        }
        Command::SplitCoins(call) => {
            shift(&mut call.coin)?;
            for arg in &mut call.amounts {
                shift(arg)?;
            }
        }
        Command::MergeCoins(call) => {
            shift(&mut call.coin)?;
            for arg in &mut call.coins_to_merge {
                shift(arg)?;
            }
        }
        _ => bail!("unexpected command in Walrus storage transaction"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, crate::sui::types::MoveCall};

    #[test]
    fn all_storage_calls_use_the_limited_coin_and_results_keep_their_dependencies() {
        let call = |name: &str, arguments: Vec<Argument>| {
            Command::MoveCall(MoveCall {
                package: Address::TWO,
                module: "system".parse().unwrap(),
                function: name.parse().unwrap(),
                type_arguments: vec![],
                arguments,
            })
        };
        let mut ptb = ProgrammableTransaction {
            inputs: vec![Input::Pure(vec![])],
            commands: vec![
                call("reserve_space", vec![Argument::Input(0)]),
                call(
                    "register_blob",
                    vec![Argument::Result(0), Argument::Input(0)],
                ),
                Command::TransferObjects(TransferObjects {
                    objects: vec![Argument::Result(1)],
                    address: Argument::Input(0),
                }),
            ],
        };
        cap_payment(&mut ptb, Address::TWO, 123).unwrap();
        assert_eq!(ptb.inputs[1], Input::Pure(bcs::to_bytes(&123u64).unwrap()));
        let Command::MoveCall(register) = &ptb.commands[2] else {
            panic!()
        };
        assert_eq!(
            register.arguments,
            vec![Argument::Result(1), Argument::NestedResult(0, 0)]
        );
        let Command::TransferObjects(transfer) = &ptb.commands[3] else {
            panic!()
        };
        assert_eq!(transfer.objects, vec![Argument::Result(2)]);
        let Command::TransferObjects(change) = &ptb.commands[4] else {
            panic!()
        };
        assert_eq!(change.objects, vec![Argument::NestedResult(0, 0)]);
    }

    #[test]
    fn coin_preparation_keeps_its_dependencies_when_the_budget_is_inserted() {
        use crate::sui::types::MergeCoins;
        let mut ptb = ProgrammableTransaction {
            inputs: vec![Input::Pure(vec![])],
            commands: vec![
                Command::SplitCoins(SplitCoins {
                    coin: Argument::Input(0),
                    amounts: vec![Argument::Input(0)],
                }),
                Command::MergeCoins(MergeCoins {
                    coin: Argument::Input(0),
                    coins_to_merge: vec![Argument::NestedResult(0, 0)],
                }),
                Command::MoveCall(MoveCall {
                    package: Address::TWO,
                    module: "system".parse().unwrap(),
                    function: "extend_blob".parse().unwrap(),
                    type_arguments: vec![],
                    arguments: vec![Argument::Input(0)],
                }),
            ],
        };
        let preparation = ptb.commands[..2].to_vec();
        cap_payment(&mut ptb, Address::TWO, 123).unwrap();
        assert_eq!(ptb.commands[..2], preparation);
        let Command::MoveCall(extend) = &ptb.commands[3] else {
            panic!("expected extension")
        };
        assert_eq!(extend.arguments.last(), Some(&Argument::NestedResult(2, 0)));
    }

    #[test]
    fn unexpected_payment_sources_are_rejected() {
        let payment = |arguments| {
            Command::MoveCall(MoveCall {
                package: Address::TWO,
                module: "system".parse().unwrap(),
                function: "reserve_space".parse().unwrap(),
                type_arguments: vec![],
                arguments,
            })
        };
        for commands in [
            vec![],
            vec![payment(vec![])],
            vec![payment(vec![Argument::Gas])],
            vec![
                payment(vec![Argument::Input(0)]),
                payment(vec![Argument::Input(1)]),
            ],
        ] {
            let mut ptb = ProgrammableTransaction {
                inputs: vec![],
                commands,
            };
            assert!(cap_payment(&mut ptb, Address::TWO, 123).is_err());
        }
    }
}
