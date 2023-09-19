use lazy_static::lazy_static;
use mojitoswap_pool::tick_math;
use radix_engine::transaction::{TransactionReceipt, TransactionResult};
use regex::Regex;
use scrypto::prelude::*;
use scrypto_unit::*;
use std::collections::HashMap;
use transaction::{builder::ManifestBuilder, model::TransactionManifestV1};

/**
 * An account used for testing purposes
 */
struct Account {
    pub addr: ComponentAddress,
    pub pub_key: Secp256k1PublicKey,
}

/**
 * A test scenario context for the pool component, we keep some state here needed for testing
 */
struct Context {
    runner: DefaultTestRunner,
    admin: Account,
    _admin_badge_addr: ResourceAddress,
    moj_addr: ResourceAddress,
    usdt_addr: ResourceAddress,
    pool_addr: ComponentAddress,
    position_nft_addr: ResourceAddress,
}

impl Context {
    /**
     * Creates a context containing:
     * - two fungible resources: MOJ and USDT
     * - an admin account that owns these resources (it acts also as faucet, giving tokens to other accounts created later
     * - a pool with the given fee and sqrt_price, the above account acts also the admin pool
     */
    pub fn new(
        fee: Decimal,
        sqrt_price: Decimal,
        low_sqrt_price: Decimal,
        high_sqrt_price: Decimal,
        moj_amount: Decimal,
        usdt_amount: Decimal,
    ) -> Self {
        let mut runner = TestRunnerBuilder::new().build();

        let package_addr = runner.compile_and_publish(this_package!());

        let (pub_key, _priv_key, addr) = runner.new_allocated_account();
        let admin = Account { addr, pub_key };

        let mut moj_token_info: BTreeMap<String, String> = BTreeMap::new();
        moj_token_info.insert("name".to_string(), "Mojito finance".to_string());
        moj_token_info.insert("symbol".to_string(), "MOJ".to_string());

        let mut usdt_token_info: BTreeMap<String, String> = BTreeMap::new();
        usdt_token_info.insert("name".to_string(), "Teather USD".to_string());
        usdt_token_info.insert("symbol".to_string(), "USDT".to_string());

        let admin_res_manif = ManifestBuilder::new()
            .new_token_fixed(
                OwnerRole::None,
                metadata!(
                    init {
                        "name" => "Mojito finance".to_owned(), locked;
                        "symbol" => "MOJ".to_owned(), locked;
                    }
                ),
                dec!("10000000"),
            )
            .new_token_fixed(
                OwnerRole::None,
                metadata!(
                    init {
                        "name" => "Teather USD".to_owned(), locked;
                        "symbol" => "USDT".to_owned(), locked;
                    }
                ),
                dec!("10000000"),
            )
            .new_badge_fixed(
                OwnerRole::None,
                metadata!(
                    init {
                        "name" => "Admin badge".to_owned(), locked;
                    }
                ),
                Decimal::one(),
            )
            .call_method(
                admin.addr,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();

        let admin_res_receipt = execute_manif(&mut runner, admin_res_manif, vec![&pub_key]);

        let result = admin_res_receipt.expect_commit_success();

        let usdt_addr: ResourceAddress = result.new_resource_addresses()[0];
        let moj_addr: ResourceAddress = result.new_resource_addresses()[1];
        let admin_badge_addr: ResourceAddress = result.new_resource_addresses()[2];

        let new_pool_manif = ManifestBuilder::new()
            .withdraw_from_account(admin.addr, moj_addr, moj_amount)
            .withdraw_from_account(admin.addr, usdt_addr, usdt_amount)
            .take_from_worktop(moj_addr, moj_amount, "moj_bucket")
            .take_from_worktop(usdt_addr, usdt_amount, "usdt_bucket")
            .call_function_with_name_lookup(package_addr, "Pool", "new", |lookup| {
                (
                    moj_addr,
                    usdt_addr,
                    fee,
                    sqrt_price,
                    admin_badge_addr,
                    low_sqrt_price,
                    high_sqrt_price,
                    lookup.bucket("moj_bucket"),
                    lookup.bucket("usdt_bucket"),
                )
            })
            .call_method(
                admin.addr,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();
        let new_pool_receipt = runner.execute_manifest_ignoring_fee(
            new_pool_manif,
            vec![NonFungibleGlobalId::from_public_key(&pub_key)],
        );
        println!("{:?}\n", new_pool_receipt);
        let result = new_pool_receipt.expect_commit_success();

        let pool_addr: ComponentAddress = result.new_component_addresses()[0];
        let pos_nft_badge_addr: ResourceAddress = result.new_resource_addresses()[1];

        Self {
            runner,
            admin,
            usdt_addr,
            moj_addr,
            _admin_badge_addr: admin_badge_addr,
            pool_addr,
            position_nft_addr: pos_nft_badge_addr,
        }
    }

    /**
     * Creates a new account used for testing having the given amounts of MOJ and USDT
     */
    pub fn new_account_with_moj_and_usdt(
        &mut self,
        moj_amount: Decimal,
        usdt_amount: Decimal,
    ) -> Account {
        let (account_pub_key, _account_priv_key, account_addr) =
            self.runner.new_allocated_account();

        let account_amount_manif = ManifestBuilder::new()
            .withdraw_from_account(self.admin.addr, self.moj_addr, moj_amount)
            .withdraw_from_account(self.admin.addr, self.usdt_addr, usdt_amount)
            .call_method(
                account_addr,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();

        execute_manif(
            &mut self.runner,
            account_amount_manif,
            vec![&self.admin.pub_key, &account_pub_key],
        );

        Account {
            addr: account_addr,
            pub_key: account_pub_key,
        }
    }

    /**
     * Adds a new position to the pool, position is owned by the given account
     */
    pub fn add_pos(
        &mut self,
        account: &Account,
        moj_amount: Decimal,
        usdt_amount: Decimal,
        low_tick: i32,
        high_tick: i32,
    ) -> TransactionReceipt {
        let add_pos_manif = ManifestBuilder::new()
            .withdraw_from_account(account.addr, self.moj_addr, moj_amount)
            .withdraw_from_account(account.addr, self.usdt_addr, usdt_amount)
            .take_from_worktop(self.moj_addr, moj_amount, "moj_bucket")
            .take_from_worktop(self.usdt_addr, usdt_amount, "usdt_bucket")
            .call_method_with_name_lookup(self.pool_addr, "add_pos", |lookup| {
                (
                    lookup.bucket("moj_bucket"),
                    lookup.bucket("usdt_bucket"),
                    tick_math::sqrt_price_at_tick(low_tick),
                    tick_math::sqrt_price_at_tick(high_tick),
                )
            })
            .assert_worktop_contains(self.moj_addr, Decimal::zero())
            .assert_worktop_contains(self.usdt_addr, Decimal::zero())
            .assert_worktop_contains(self.position_nft_addr, Decimal::one())
            .call_method(
                account.addr,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();

        let add_pos_receipt = self.runner.execute_manifest_ignoring_fee(
            add_pos_manif,
            vec![NonFungibleGlobalId::from_public_key(&account.pub_key)],
        );
        println!("{:?}\n", add_pos_receipt);

        add_pos_receipt.expect_commit_success();
        add_pos_receipt
    }

    /**
     * Adds more liquidity to the position owned by the given account (for the moment this test utility allows only for a position per account)
     * Unfortunatelly we can't check the liquidity on the returned position NFT at the worktop level
     */
    pub fn add_liq(
        &mut self,
        account: &Account,
        usdt_amount: Decimal,
        moj_amount: Decimal,
    ) -> TransactionReceipt {
        let add_liq_manif = ManifestBuilder::new()
            .withdraw_from_account(account.addr, self.moj_addr, moj_amount)
            .withdraw_from_account(account.addr, self.usdt_addr, usdt_amount)
            .take_from_worktop(self.moj_addr, moj_amount, "moj_bucket")
            .take_from_worktop(self.usdt_addr, usdt_amount, "usdt_bucket")
            .create_proof_from_account_of_non_fungible(
                account.addr,
                self.pos_nft_badge_id(account.addr),
            )
            .create_proof_from_auth_zone_of_amount(self.position_nft_addr, Decimal::one(), "proof")
            .call_method_with_name_lookup(self.pool_addr, "add_liq", |lookup| {
                (
                    lookup.bucket("moj_bucket"),
                    lookup.bucket("usdt_bucket"),
                    lookup.proof("proof"),
                )
            })
            .assert_worktop_contains(self.moj_addr, Decimal::zero())
            .assert_worktop_contains(self.usdt_addr, Decimal::zero())
            .call_method(
                account.addr,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();

        let add_liq_receipt = self.runner.execute_manifest_ignoring_fee(
            add_liq_manif,
            vec![NonFungibleGlobalId::from_public_key(&account.pub_key)],
        );
        println!("{:?}\n", add_liq_receipt);

        add_liq_receipt.expect_commit_success();
        add_liq_receipt
    }

    fn pos_nft_badge_id(&mut self, account_addr: ComponentAddress) -> NonFungibleGlobalId {
        let vaults = self
            .runner
            .get_component_vaults(account_addr, self.position_nft_addr);
        let nft_local_id = self
            .runner
            .inspect_non_fungible_vault(vaults[0])
            .unwrap()
            .1
            .next()
            .unwrap();
        NonFungibleGlobalId::new(self.position_nft_addr, nft_local_id)
    }

    /**
     * Adds the fees accumulated by the account's position to liquidity
     */
    pub fn add_accumulated_fees_to_liq(&mut self, account: &Account) -> TransactionReceipt {
        let add_liq_manif = ManifestBuilder::new()
            .create_proof_from_account_of_non_fungible(
                account.addr,
                self.pos_nft_badge_id(account.addr),
            )
            .create_proof_from_auth_zone_of_amount(self.position_nft_addr, Decimal::one(), "proof")
            .call_method_with_name_lookup(self.pool_addr, "add_accumulated_fees_to_liq", |lookup| {
                (lookup.proof("proof"),)
            })
            .build();

        let add_liq_receipt = self.runner.execute_manifest_ignoring_fee(
            add_liq_manif,
            vec![NonFungibleGlobalId::from_public_key(&account.pub_key)],
        );
        println!("{:?}\n", add_liq_receipt);

        add_liq_receipt.expect_commit_success();
        add_liq_receipt
    }

    pub fn remove_admin_pos(
        &mut self,
        expected_moj_amount: Decimal,
        expected_usdt_amount: Decimal,
    ) -> TransactionReceipt {
        let remove_liq_manif = self.create_remove_liq_manif(
            self.admin.addr,
            expected_moj_amount,
            expected_usdt_amount,
        );
        self.execute_remove_lig_manif(remove_liq_manif, &self.admin.pub_key.clone())
    }

    /**
     * Removes the given account's position and checks that upon removal the account got the expected amounts
     */
    pub fn remove_pos(
        &mut self,
        account: &Account,
        expected_moj_amount: Decimal,
        expected_usdt_amount: Decimal,
    ) -> TransactionReceipt {
        let remove_liq_manif =
            self.create_remove_liq_manif(account.addr, expected_moj_amount, expected_usdt_amount);
        self.execute_remove_lig_manif(remove_liq_manif, &account.pub_key)
    }

    fn create_remove_liq_manif(
        &mut self,
        account_addr: ComponentAddress,
        expected_moj_amount: Decimal,
        expected_usdt_amount: Decimal,
    ) -> TransactionManifestV1 {
        ManifestBuilder::new()
            .create_proof_from_account_of_non_fungible(
                account_addr,
                self.pos_nft_badge_id(account_addr),
            )
            .create_proof_from_auth_zone_of_amount(self.position_nft_addr, Decimal::one(), "proof")
            .call_method_with_name_lookup(self.pool_addr, "remove_pos", |lookup| {
                (lookup.proof("proof"),)
            })
            .assert_worktop_contains(self.moj_addr, expected_moj_amount)
            .assert_worktop_contains(self.usdt_addr, expected_usdt_amount)
            .call_method(
                account_addr,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build()
    }

    fn execute_remove_lig_manif(
        &mut self,
        remove_liq_manif: TransactionManifestV1,
        account_pub_key: &Secp256k1PublicKey,
    ) -> TransactionReceipt {
        let remove_liq_receipt = self.runner.execute_manifest_ignoring_fee(
            remove_liq_manif,
            vec![NonFungibleGlobalId::from_public_key(account_pub_key)],
        );
        println!("{:?}\n", remove_liq_receipt);

        remove_liq_receipt.expect_commit_success();
        remove_liq_receipt
    }

    /**
     * Collects the fees accumulated by the given account's position and checks the expected amounts of fees
     */
    pub fn collect_fees(
        &mut self,
        account: &Account,
        expected_moj_amount: Decimal,
        expected_usdt_amount: Decimal,
    ) -> TransactionReceipt {
        let collect_fees_manif = ManifestBuilder::new()
            .create_proof_from_account_of_non_fungible(
                account.addr,
                self.pos_nft_badge_id(account.addr),
            )
            .create_proof_from_auth_zone_of_amount(self.position_nft_addr, Decimal::one(), "proof")
            .call_method_with_name_lookup(self.pool_addr, "collect_fees", |lookup| {
                (lookup.proof("proof"),)
            })
            .assert_worktop_contains(self.moj_addr, expected_moj_amount)
            .assert_worktop_contains(self.usdt_addr, expected_usdt_amount)
            .call_method(
                account.addr,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();

        let remove_liq_receipt = self.runner.execute_manifest_ignoring_fee(
            collect_fees_manif,
            vec![NonFungibleGlobalId::from_public_key(&account.pub_key)],
        );
        println!("{:?}\n", remove_liq_receipt);

        remove_liq_receipt.expect_commit_success();
        remove_liq_receipt
    }

    /**
     * Swaps the given amount of MOJ taken from the given account to USDT. Also check the expected amount of USDT.
     */
    pub fn swap_moj_for_usdt(
        &mut self,
        account: &Account,
        moj_amount: Decimal,
        exp_usdt_amount: Decimal,
    ) -> TransactionReceipt {
        self.swap(
            account,
            self.moj_addr,
            moj_amount,
            self.usdt_addr,
            exp_usdt_amount,
        )
    }

    /**
     * Swaps the given amount of USDT taken from the given account to MOJ. Also check the expected amount of MOJ.
     */
    pub fn swap_usdt_for_moj(
        &mut self,
        account: &Account,
        usdt_amount: Decimal,
        exp_moj_amount: Decimal,
    ) -> TransactionReceipt {
        self.swap(
            account,
            self.usdt_addr,
            usdt_amount,
            self.moj_addr,
            exp_moj_amount,
        )
    }
    /**
     * Executes a swap for the given resource address and amount. It expects the given expected resurce address and amount.
     */
    fn swap(
        &mut self,
        account: &Account,
        token_addr: ResourceAddress,
        token_amount: Decimal,
        expected_token_addr: ResourceAddress,
        expected_token_amount: Decimal,
    ) -> TransactionReceipt {
        let swap_manif = ManifestBuilder::new()
            .withdraw_from_account(account.addr, token_addr, token_amount)
            .take_from_worktop(token_addr, token_amount, "token_bucket")
            .call_method_with_name_lookup(self.pool_addr, "swap", |lookup| {
                (lookup.bucket("token_bucket"),)
            })
            .assert_worktop_contains(token_addr, Decimal::zero())
            .assert_worktop_contains(expected_token_addr, expected_token_amount)
            .call_method(
                account.addr,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();
        let swap_receipt = self.runner.execute_manifest_ignoring_fee(
            swap_manif,
            vec![NonFungibleGlobalId::from_public_key(&account.pub_key)],
        );
        println!("{:?}\n", swap_receipt);
        swap_receipt.expect_commit_success();
        swap_receipt
    }
}

/**
 * Executes a given manifest and expects to be successful
 */
fn execute_manif(
    runner: &mut DefaultTestRunner,
    manif: TransactionManifestV1,
    pub_keys: Vec<&Secp256k1PublicKey>,
) -> TransactionReceipt {
    let receipt = runner.execute_manifest_ignoring_fee(
        manif,
        pub_keys
            .iter()
            .map(|pub_key| NonFungibleGlobalId::from_public_key(*pub_key))
            .collect::<Vec<_>>(),
    );
    println!("{:?}\n", receipt);
    receipt.expect_commit_success();
    receipt
}

/**
 * Add position.
 *
 * We test that upon adding a position the internal state of the pool is as expected. For this we read the logs from the transaction receipt.
 */
#[test]
fn add_pos() {
    let mut context = Context::new(
        Decimal::zero(),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("10000"), dec!("10000"));
    let add_pos_receipt = context.add_pos(&account, dec!("10000"), dec!("10000"), -1000, 1000);

    //todo refactor/extract somewhere the code bellow in a more generic way, so we can use this in every test if we want to check an expected internal state.
    lazy_static! {
        static ref RE_KEY_TO_LOG: Regex = Regex::new(r"### (.*)=(.*)").unwrap();
        static ref RE_POSITION_IDS: Regex = Regex::new(r"\[(.*)\]").unwrap();
        static ref RE_POS_ID: Regex = Regex::new(r"\{\S*\}").unwrap();
    }

    if let TransactionResult::Commit(result) = &add_pos_receipt.result {
        let key_to_log: HashMap<String, String> = result
            .application_logs
            .iter()
            .filter(|level_and_log| RE_KEY_TO_LOG.is_match(level_and_log.1.as_str()))
            .map(|level_and_log| level_and_log.1.as_str())
            .flat_map(|mess| RE_KEY_TO_LOG.captures(mess))
            .map(|captures| (captures[1].to_string(), captures[2].to_string()))
            .collect();

        println!("{:?}", key_to_log);

        assert_eq!(
            key_to_log.get("Vault0"),
            Some(&String::from("19999.999999999999939578"))
        );
        assert_eq!(key_to_log.get("Vault1"), Some(&String::from("20000")));
        assert_eq!(
            key_to_log.get("Life liq"),
            Some(&String::from("410103.325362140397360716"))
        );
        assert_eq!(
            key_to_log.get("Used ticks"),
            Some(&String::from("{-1000, 1000}"))
        );
        assert_eq!(
            key_to_log.get("Positions"),
            Some(&String::from(
                "[Position { liq: 205051.662681070198680358, low_tick: -1000, high_tick: 1000, range_fee0: 0, range_fee1: 0 }, Position { liq: 205051.662681070198680358, low_tick: -1000, high_tick: 1000, range_fee0: 0, range_fee1: 0 }]"
            ))
        );

        assert_eq!(
            key_to_log.get("Position NFT liq"),
            Some(&String::from("205051.662681070198680358"))
        );

        let ids = key_to_log
            .get("Position ids")
            .and_then(|raw_ids| RE_POSITION_IDS.captures(raw_ids).map(|c| c[1].to_string()))
            .unwrap();
        let id_set: HashSet<&str> = RE_POS_ID
            .find_iter(ids.as_str())
            .map(|id| id.as_str())
            .collect();
        assert!(id_set.contains(key_to_log.get("Position id").map(|l| l.as_str()).unwrap()));
    }
}

/**
 * Add liq.
 *
 * For the moment this is just a smoke test, that we can add liq to pool
 */
#[test]
fn add_liq() {
    let mut context = Context::new(
        Decimal::zero(),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("20000"), dec!("20000"));
    context.add_pos(&account, dec!("10000"), dec!("10000"), -1000, 1000);
    let _add_liq_receipt = context.add_liq(&account, dec!("10000"), dec!("10000"));
    // to do check pool internal state to have the expected state
}

/**
 * Add/remove position.
 *
 * Add a position, then remove it. Expect we get the same amount we put.
 */
#[test]
fn scenario_1() {
    let mut context = Context::new(
        Decimal::zero(),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("10000"), dec!("10000"));
    context.add_pos(&account, dec!("10000"), dec!("10000"), -1000, 1000);
    let _remove_pos_receipt = context.remove_pos(
        &account,
        dec!("9999.99999999999985322"),
        dec!("9999.999999999999999999"),
    );
    // to do check pool internal state to have the initial state
}

/**
 * Single range swap.
 *
 * Given a pool with fee=0.01, sqrt_price=1 and a position=[10000 MOJ + 10000 USDT, -1000, 1000]
 *
 * Test that after a MOJ swap we get the expected amount of USDT.
 */
#[test]
fn scenario_2() {
    let mut context = Context::new(
        dec!("0.01"),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("15000"), dec!("10000"));
    context.add_pos(&account, dec!("10000"), dec!("10000"), -1000, 1000);
    let _swap_receipt =
        context.swap_moj_for_usdt(&account, dec!("5000"), dec!("4833.322352370076335998"));
    // to do check pool internal state to have the expected state
}

/**
 * Single range swap.
 *
 * Given a pool with fee=0.01, sqrt_price=1 and 10 X position=[1000 MOJ + 1000 USDT, -1000, 1000]
 *
 * Test that after a MOJ swap we get the expected amount of USDT.
 */
#[test]
fn scenario_3() {
    let mut context = Context::new(
        dec!("0.01"),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("15000"), dec!("10000"));
    for _i in 0..10 {
        context.add_pos(&account, dec!("1000"), dec!("1000"), -1000, 1000);
    }
    let _swap_receipt =
        context.swap_moj_for_usdt(&account, dec!("5000"), dec!("4833.322352370076335998"));
    // to do check pool internal state to have the expected state
}

/**
 * Single range swap.
 *
 * Given a pool with fee=0.01, sqrt_price=1 and a position=[10000 MOJ + 10000 USDT, -1000, 1000]
 *
 * Test if first we swap 10000 MOJ to USDT and then we swap the USDT back, we get the same amoun or bit less.
 */
#[test]
fn scenario_4() {
    let mut context = Context::new(
        Decimal::zero(),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("20000"), dec!("10000"));
    context.add_pos(&account, dec!("10000"), dec!("10000"), -1000, 1000);
    let _swap1_receipt =
        context.swap_moj_for_usdt(&account, dec!("10000"), dec!("9761.963321966572685348"));
    let _swap2_receipt = context.swap_usdt_for_moj(
        &account,
        dec!("9761.963321966572685348"),
        dec!("9999.999999999999589896"),
    );
}

/**
 * Single range swap.
 *
 * Given a pool with fee=0, sqrt_price=1 and a position=[10000 MOJ + 10000 USDT, -1000, 1000]
 *
 * Test that we can swap ~10512 MOJ right to the end of the range(tick -1000) and that the expected amount of USDT is returned
 */
#[test]
fn scenario_5() {
    let mut context = Context::new(
        Decimal::zero(),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("30000"), dec!("30000"));
    context.add_pos(&account, dec!("10000"), dec!("10000"), -1000, 1000);
    let _swap1_receipt = context.swap_moj_for_usdt(
        &account,
        dec!("10512.684683767608857909"),
        dec!("9999.999999999957144202"),
    );
}

/**
 * Limit order & multiple range swap.
 *
 * Given a pool with fee=0, sqrt_price=1 and a position=[10000 MOJ + 10000 USDT, -1000, 1000]
 *
 * Test that if:
 * - an account adds a position=[1000 MOJ, 199, 200], this can act as limit order saying: sell 1000MOJ at price 1.02. THe range [199, 200] corresponding to sqrt_prices=[1.009999163397141405, 1.010049662092875426] => price 1.02
 * - the price moves past the position range
 * - the account holding the limit order position remove it
 *
 * Then the account gets the expected amount of USDT: ~1020 USDT
 */
#[test]
fn scenario_6() {
    let mut context = Context::new(
        Decimal::zero(),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("20000"), dec!("20000"));
    context.add_pos(&account, dec!("10000"), dec!("10000"), -1000, 1000);

    let account2 = context.new_account_with_moj_and_usdt(dec!("20000"), dec!("20000"));
    context.add_pos(&account2, dec!("1000"), Decimal::zero(), 199, 200);

    context.swap_usdt_for_moj(&account, dec!("8000"), dec!("7750.081465536550594191"));
    context.remove_pos(&account2, Decimal::zero(), dec!("1020.149313703371480602"));
}

/**
 * Single range swap.
 *
 * Given a pool with fee=0.01, sqrt_price=1 and a position=[10000 MOJ + 10000 USDT, -1000, 1000]
 *
 * Test that after a USDT swap we get the expected amount of MOJ.
 */
#[test]
fn scenario_7() {
    let mut context = Context::new(
        dec!("0.01"),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("10000"), dec!("15000"));
    context.add_pos(&account, dec!("10000"), dec!("10000"), -1000, 1000);
    let _swap_receipt =
        context.swap_usdt_for_moj(&account, dec!("5000"), dec!("4833.322352370076335998"));
    // to do check pool internal state to have the expected state
}

/**
 * Single range swap.
 *
 * Given a pool with fee=0.01, sqrt_price=1 and 10 X position=[1000 MOJ + 1000 USDT, -1000, 1000]
 *
 * Test that after a USDT swap we get the expected amount of MOJ. The swap happens on a single range.
 */
#[test]
fn scenario_8() {
    let mut context = Context::new(
        dec!("0.01"),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("10000"), dec!("15000"));
    for _i in 0..10 {
        context.add_pos(&account, dec!("1000"), dec!("1000"), -1000, 1000);
    }
    let _swap_receipt =
        context.swap_usdt_for_moj(&account, dec!("5000"), dec!("4833.322352370076335998"));
    // to do check pool internal state to have the expected state
}

/**
 * Add liquidity to position.
 *
 * Given a pool with fee=0.01, sqrt_price= sqrt_price at tick 3000 and a position=[10000 MOJ + 10000 USDT, 2000, 4000]
 *
 * If:
 * We swap back and forth 5000 MOJ just to accumulate some fees for the position.
 * And then add the accumulated fees to position.
 *
 * Then:
 * We can collect just a small amount of MOJ, that couldn't be added to liquidity.
 * We can get back the right amount of MOJ and USDT if we remove the position
 */
#[test]
fn scenario_9() {
    let mut context = Context::new(
        dec!("0.01"),
        tick_math::sqrt_price_at_tick(3000),
        tick_math::sqrt_price_at_tick(2000),
        tick_math::sqrt_price_at_tick(4000),
        dec!("10000"),
        dec!("10000"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("15000"), dec!("15000"));
    let _add_pos_receipt = context.add_pos(&account, dec!("10000"), dec!("10000"), 2000, 4000);
    context.swap_moj_for_usdt(&account, dec!("5000"), dec!("6574.583002247458274286"));
    context.swap_usdt_for_moj(
        &account,
        dec!("6574.583002247458274286"),
        dec!("4901.285751004560687604"),
    );
    let _add_liq_receipt = context.add_accumulated_fees_to_liq(&account);
    let _acc_fees_receipt =
        context.collect_fees(&account, dec!("0.487730419238072792"), Decimal::zero());
    context.remove_pos(
        &account,
        dec!("7457.164320366552918974"),
        dec!("9999.999999999999823508"),
    );
}

/**
 * Collect fees.
 *
 * Given a pool with fee=0.01, sqrt_price=1 and:
 * - position1=[10000 MOJ + 10000 USDT, -1000, 1000]
 * - position2=[10000 MOJ + 10000 USDT, 2000, 1000]
 *
 * If there is a 5000 MOJ swap on range [-1000, 1000]
 *
 * Then only position1 accumulates ~50 MOJ fees, position2 accumulates 0
 */
#[test]
fn scenario_10() {
    let mut context = Context::new(
        dec!("0.01"),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    context.remove_admin_pos(
        dec!("9999.999999999999969789"),
        dec!("9999.999999999999999999"),
    );
    let account1 = context.new_account_with_moj_and_usdt(dec!("15000"), dec!("10000"));
    context.add_pos(&account1, dec!("10000"), dec!("10000"), -1000, 1000);
    let account2 = context.new_account_with_moj_and_usdt(dec!("15000"), dec!("10000"));
    context.add_pos(&account2, dec!("10000"), dec!("10000"), 2000, 4000);
    context.swap_moj_for_usdt(&account1, dec!("5000"), dec!("4833.322352370076335998"));
    let _acc1_fees_receipt =
        context.collect_fees(&account1, dec!("49.999999999999999999"), Decimal::zero());
    let _acc2_fees_receipt = context.collect_fees(&account2, Decimal::zero(), Decimal::zero());
}

/**
 * Collect fees.
 *
 * Given a pool with fee=0.01, sqrt_price=1 and:
 * - position=[10000 MOJ + 10000 USDT, -1000, 1000]
 *
 * If there is a 5000 MOJ swap on range [-1000, 1000]
 *
 * Then position accumulates ~50 MOJ fees that can be collected only once
 */
#[test]
fn scenario_11() {
    let mut context = Context::new(
        dec!("0.01"),
        tick_math::sqrt_price_at_tick(3000),
        tick_math::sqrt_price_at_tick(2000),
        tick_math::sqrt_price_at_tick(4000),
        dec!("10000"),
        dec!("10000"),
    );
    context.remove_admin_pos(
        dec!("7408.293322976009370503"),
        dec!("9999.999999999999999999"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("15000"), dec!("10000"));
    let _add_pos_receipt = context.add_pos(&account, dec!("10000"), dec!("10000"), 2000, 4000);
    context.swap_moj_for_usdt(&account, dec!("5000"), dec!("6470.845461125638454765"));
    context.collect_fees(&account, dec!("49.999999999999855277"), Decimal::zero());
    context.collect_fees(&account, Decimal::zero(), Decimal::zero());
}

/**
 * Collect fees.
 *
 * Given a pool with fee=0.01, sqrt_price=1 and:
 * - position1=[10000 MOJ + 10000 USDT, -1000, 1000]
 * - position2=[10000 MOJ + 10000 USDT, -2000, 2000]
 *
 * If there is a 5000 MOJ swap on range [-1000, 1000]
 *
 * Then we would expect that position1 accumulates ~2/3 of the ~50 fees or ~33 and position2 accumulates ~1/3 or ~16
 *
 * In reality this ratios are bit skewed. The real ratio of fees is visible in the net liquidity on the positions:
 *
 * Position1 { liq: 205051.662681070198680358, low_tick: -1000, high_tick: 1000}, Position2 { liq: 105088.315200116552078393, low_tick: -2000, high_tick: 2000}]
 */
#[test]
fn scenario_12() {
    let mut context = Context::new(
        dec!("0.01"),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    context.remove_admin_pos(
        dec!("9999.999999999999969789"),
        dec!("9999.999999999999999999"),
    );
    let account1 = context.new_account_with_moj_and_usdt(dec!("15000"), dec!("10000"));
    context.add_pos(&account1, dec!("10000"), dec!("10000"), -1000, 1000);

    let account2 = context.new_account_with_moj_and_usdt(dec!("10000"), dec!("10000"));
    context.add_pos(&account2, dec!("10000"), dec!("10000"), -2000, 2000);

    context.swap_moj_for_usdt(&account1, dec!("5000"), dec!("4833.322352370086235614"));

    let _acc1_fees_receipt =
        context.collect_fees(&account1, dec!("33.057921794207346548"), Decimal::zero());
    let _acc2_fees_receipt =
        context.collect_fees(&account2, dec!("16.942078205792448383"), Decimal::zero());
}

/**
 * Collect fees.
 *
 * Given a pool with fee=0.01, sqrt_price=1 and:
 * - position1=[10000 MOJ + 10000 USDT, -1000, 1000]
 * - position2=[100000 MOJ + 100000 USDT, -10000, 10000]
 *
 * If there is a 5000 MOJ swap on range [-1000, 1000]
 *
 * Then we would expect that position1 accumulates ~1/2 of the ~50 fees or ~25 and position2 accumulates the sane
 *
 * In reality this ratios are bit skewed. The real ratio of fees is visible in the net liquidity on the positions:
 *
 * Position1 { liq: 205051.662681070198680358, low_tick: -1000, high_tick: 1000}, Position2 { liq: 254159.202345836056361321, low_tick: -10000, high_tick: 10000}
 */
#[test]
fn scenario_13() {
    let mut context = Context::new(
        dec!("0.01"),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    context.remove_admin_pos(
        dec!("9999.999999999999969789"),
        dec!("9999.999999999999999999"),
    );
    let account1 = context.new_account_with_moj_and_usdt(dec!("15000"), dec!("10000"));
    context.add_pos(&account1, dec!("10000"), dec!("10000"), -1000, 1000);
    let account2 = context.new_account_with_moj_and_usdt(dec!("100000"), dec!("100000"));
    context.add_pos(&account2, dec!("100000"), dec!("100000"), -10000, 10000);
    context.swap_moj_for_usdt(&account1, dec!("5000"), dec!("4891.685469231850922674"));
    let _acc1_fees_receipt =
        context.collect_fees(&account1, dec!("22.326525600505424713"), Decimal::zero());
    let _acc2_fees_receipt =
        context.collect_fees(&account2, dec!("27.673474399494349869"), Decimal::zero());
}

/**
 * Multiple range swap.
 *
 * Swap up the ticks by receiving USDT for MOJ, then down the ticks by giving back the amount of MOJ we first got. We should get back the same amount of USDT, or a bit less.
 *
 * Given:
 * - account=[moj=100000, usdt=100000]
 * - sqrt_price=1 MOJ/USDT
 * - pool_fee=0
 * - position1=[10000 MOJ, 10000 USDT, -1000, 1000]
 * - position2=[10000 MOJ, 0, 100]
 * - position3=[10000 MOJ, 100, 200]
 *
 * Assert that swapping 20000 usdt results in ~19647 moj
 * Assert that swapping ~19647 usdt results in ~20000 moj
 */
#[test]
fn scenario_14() {
    let mut context = Context::new(
        Decimal::zero(),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    context.remove_admin_pos(
        dec!("9999.999999999999969789"),
        dec!("9999.999999999999999999"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("100000"), dec!("100000"));
    context.add_pos(&account, dec!("10000"), dec!("10000"), -1000, 1000);
    context.add_pos(&account, dec!("10000"), Decimal::zero(), 100, 200);
    context.add_pos(&account, dec!("10000"), Decimal::zero(), 200, 300);
    context.swap_usdt_for_moj(&account, dec!("20000"), dec!("19647.863604192115415544"));
    context.swap_moj_for_usdt(
        &account,
        dec!("19647.863604192115415544"),
        dec!("19999.999999999994361019"),
    );
}

/**
 * Single range swap. Liquidity limit.
 *
 * Given a pool with fee=0, sqrt_price=1 and a position=[10000 MOJ + 10000 USDT, -1000, 1000]
 *
 * Test that if we try to swap 11000 MOJ, we can swap only ~10512 MOJ right to the end of the range(tick -1000), that the expected amount of USDT is returned (~10000 USDT) and the reminder ~ 488MOJ are returned back, as we have no more liquidity to swap it.
 */
#[test]
fn scenario_15() {
    let mut context = Context::new(
        Decimal::zero(),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    context.remove_admin_pos(
        dec!("9999.999999999999969789"),
        dec!("9999.999999999999999999"),
    );
    let account = context.new_account_with_moj_and_usdt(dec!("30000"), dec!("30000"));
    context.add_pos(&account, dec!("10000"), dec!("10000"), -1000, 1000);
    let _swap1_receipt =
        context.swap_moj_for_usdt(&account, dec!("11000"), dec!("9999.999999999957144202"));
}

/**
 * Collect fees.
 *
 * Given:
 * - pool with fee=0.01, sqrt_price=1
 * - position1=[10000 MOJ, 10000 USDT, -1000, 1000]
 * - position2=[10000 MOJ, 10000 USDT, -1000, 1000]
 *
 * If: there is a 5000 MOJ swap in range [-1000, 1000]
 *
 * Then: position 1 and position 2 split equally the 50 MOJ fee
 */
#[test]
fn scenario_16() {
    let mut context = Context::new(
        dec!("0.01"),
        Decimal::one(),
        tick_math::sqrt_price_at_tick(-1000),
        tick_math::sqrt_price_at_tick(1000),
        dec!("10000"),
        dec!("10000"),
    );
    context.remove_admin_pos(
        dec!("9999.999999999999969789"),
        dec!("9999.999999999999999999"),
    );
    let account1 = context.new_account_with_moj_and_usdt(dec!("100000"), dec!("100000"));
    let account2 = context.new_account_with_moj_and_usdt(dec!("100000"), dec!("100000"));
    context.add_pos(&account1, dec!("10000"), dec!("10000"), -1000, 1000);
    context.add_pos(&account2, dec!("10000"), dec!("10000"), -1000, 1000);
    context.swap_moj_for_usdt(&account1, dec!("5000"), dec!("4890.96541696510667305"));
    context.collect_fees(&account1, dec!("24.999999999999999999"), Decimal::zero());
    context.collect_fees(&account2, dec!("24.999999999999999999"), Decimal::zero());
}

// To be continued...
