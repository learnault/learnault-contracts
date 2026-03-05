use soroban_sdk::{symbol_short, vec, Env};

#[test]
fn test_hello() {
    let env = Env::default();
    let contract_id = env.register(crate::HelloLearnault, ());
    let client = crate::HelloLearnaultClient::new(&env, &contract_id);

    let result = client.hello(&symbol_short!("Dev"));
    assert_eq!(
        result,
        vec![
            &env,
            symbol_short!("Hello"),
            symbol_short!("Learnault"),
            symbol_short!("Dev")
        ]
    );
}

#[test]
fn test_welcome() {
    let env = Env::default();
    let contract_id = env.register(crate::HelloLearnault, ());
    let client = crate::HelloLearnaultClient::new(&env, &contract_id);

    let result = client.welcome(&symbol_short!("Alice"));
    assert_eq!(
        result,
        vec![
            &env,
            symbol_short!("Welcome"),
            symbol_short!("to"),
            symbol_short!("Learnault"),
            symbol_short!("Alice")
        ]
    );
}
