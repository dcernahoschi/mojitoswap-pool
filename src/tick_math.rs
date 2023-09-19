use scrypto::prelude::*;

//smallest decimal: 0.000000000000000001 (10 ^ -18) -> smallest tick: -828972, but effectively for us smallest price is 0.00000000000001985 with min tick -631042
//as we can't have enough precision under this values: e.g. for tick -631043 the price would be 0.000000000000019849 as we don't have enough decimal places to represent it
//we stop for now at decimal: 170141183460469231731.687303715884105727 (2^127 - 1) * 10 ^ -18 -> largest tick: 931709 -> real max 170134484377190040957.155711420855095752, but this limit can be increased
const MIN_TICK: i32 = -631042;
const MAX_TICK: i32 = 931709;

//0.00000000000001985
const MIN_PRICE: Decimal = Decimal(bnum_integer::I192::from_digits([19850, 0, 0]));
//1.000049998750062496 = √1.0001^(2^0)
const PRICE_0X1: Decimal = Decimal(bnum_integer::I192::from_digits([1000049998750062496, 0, 0]));
//1.0001 = √1.0001^(2^1)
const PRICE_0X2: Decimal = Decimal(bnum_integer::I192::from_digits([1000100000000000000, 0, 0]));
//1.00020001 = √1.0001^(2^2)
const PRICE_0X4: Decimal = Decimal(bnum_integer::I192::from_digits([1000200010000000000, 0, 0]));
//1.000400060004000093 = √1.0001^(2^3)
const PRICE_0X8: Decimal = Decimal(bnum_integer::I192::from_digits([1000400060004000093, 0, 0]));
//1.000800280056006986 = √1.0001^(2^4)
const PRICE_0X10: Decimal = Decimal(bnum_integer::I192::from_digits([1000800280056006986, 0, 0]));
//1.001601200560182014 = √1.0001^(2^5)
const PRICE_0X20: Decimal = Decimal(bnum_integer::I192::from_digits([1001601200560182014, 0, 0]));
//1.003204964963597955 = √1.0001^(2^6)
const PRICE_0X40: Decimal = Decimal(bnum_integer::I192::from_digits([1003204964963597955, 0, 0]));
//1.006420201727613800 = √1.0001^(2^7)
const PRICE_0X80: Decimal = Decimal(bnum_integer::I192::from_digits([1006420201727613800, 0, 0]));
//1.012881622445450855 = √1.0001^(2^8)
const PRICE_0X100: Decimal = Decimal(bnum_integer::I192::from_digits([1012881622445450855, 0, 0]));
//1.025929181087728853 = √1.0001^(2^9)
const PRICE_0X200: Decimal = Decimal(bnum_integer::I192::from_digits([1025929181087728853, 0, 0]));
//1.052530684607337941 = √1.0001^(2^10)
const PRICE_0X400: Decimal = Decimal(bnum_integer::I192::from_digits([1052530684607337941, 0, 0]));
//1.107820842039991493 = √1.0001^(2^11)
const PRICE_0X800: Decimal = Decimal(bnum_integer::I192::from_digits([1107820842039991493, 0, 0]));
//1.227267018058195782 = √1.0001^(2^12)
const PRICE_0X1000: Decimal = Decimal(bnum_integer::I192::from_digits([1227267018058195782, 0, 0]));
//1.506184333613455851 = √1.0001^(2^13)
const PRICE_0X2000: Decimal = Decimal(bnum_integer::I192::from_digits([1506184333613455851, 0, 0]));
//2.268591246822610072 = √1.0001^(2^14)
const PRICE_0X4000: Decimal = Decimal(bnum_integer::I192::from_digits([2268591246822610072, 0, 0]));
//5.146506245160164533 = √1.0001^(2^15)
const PRICE_0X8000: Decimal = Decimal(bnum_integer::I192::from_digits([5146506245160164533, 0, 0]));
//26.486526531472575563 = √1.0001^(2^16)
const PRICE_0X10000: Decimal = Decimal(bnum_integer::I192::from_digits([8039782457763023947, 1, 0]));
//701.536087702400664335 = √1.0001^(2^17)
const PRICE_0X20000: Decimal = Decimal(bnum_integer::I192::from_digits([559812901437702927, 38, 0]));
//492152.882348790396620919 = √1.0001^(2^18)
const PRICE_0X40000: Decimal = Decimal(bnum_integer::I192::from_digits([12197206293269057655, 26679, 0]));
//242214459604.222321943471435452 = √1.0001^(2^19)
const PRICE_0X80000: Decimal = Decimal(bnum_integer::I192::from_digits([9500346154952666812, 13130472165, 0]));
//170134484377190040957.155711420855095752
const MAX_PRICE: Decimal = Decimal(bnum_integer::I192::from_digits([4809668506064654792, 9223008878822527810, 0]));

/** 
 * By definition, sqrt_price = sqrt(1.0001) ^ tick, but tick is always a sum of powers of 2, e.g. 7 = 2^0 + 2^1 + 2^2,
 * So, sqrt_price = sqrt(1.0001) ^ (2 ^ a + 2 ^ b + ...) = sqrt(1.0001) ^ (2 ^ a) * sqrt(1.0001) ^ (2 ^ b) * ...
 * Where  a,b,... are uniques values in interval [0, 19], given the max tick value of 931709.
 * 
 * So, the algorithm bellow decompose the given tick in a power of 2 sum, and for each power of 2, it multiplies 
 * the sqrt_price with the corresponding pre-computed sqrt_price from the constants above. This is the sqrt_price we are looking for.
 */ 
pub fn sqrt_price_at_tick(tick: i32) -> Decimal {
    assert!(tick >= MIN_TICK && tick <= MAX_TICK, "Tick out of bounds.");

    let abs_tick = if tick >= 0 { tick } else { -tick };
    let mut sqrt_price = Decimal::one();

    if abs_tick & 0x1 != 0 {
        sqrt_price = sqrt_price * PRICE_0X1;
    }
    if abs_tick & 0x2 != 0 {
        sqrt_price = sqrt_price * PRICE_0X2;
    }
    if abs_tick & 0x4 != 0 {
        sqrt_price = sqrt_price * PRICE_0X4;
    }
    if abs_tick & 0x8 != 0 {
        sqrt_price = sqrt_price * PRICE_0X8;
    }
    if abs_tick & 0x10 != 0 {
        sqrt_price = sqrt_price * PRICE_0X10;
    }
    if abs_tick & 0x20 != 0 {
        sqrt_price = sqrt_price * PRICE_0X20;
    }
    if abs_tick & 0x40 != 0 {
        sqrt_price = sqrt_price * PRICE_0X40;
    }
    if abs_tick & 0x80 != 0 {
        sqrt_price = sqrt_price * PRICE_0X80;
    }
    if abs_tick & 0x100 != 0 {
        sqrt_price = sqrt_price * PRICE_0X100;
    }
    if abs_tick & 0x200 != 0 {
        sqrt_price = sqrt_price * PRICE_0X200;
    }
    if abs_tick & 0x400 != 0 {
        sqrt_price = sqrt_price * PRICE_0X400;
    }
    if abs_tick & 0x800 != 0 {
        sqrt_price = sqrt_price * PRICE_0X800;
    }
    if abs_tick & 0x1000 != 0 {
        sqrt_price = sqrt_price * PRICE_0X1000;
    }
    if abs_tick & 0x2000 != 0 {
        sqrt_price = sqrt_price * PRICE_0X2000;
    }
    if abs_tick & 0x4000 != 0 {
        sqrt_price = sqrt_price * PRICE_0X4000;
    }
    if abs_tick & 0x8000 != 0 {
        sqrt_price = sqrt_price * PRICE_0X8000;
    }
    if abs_tick & 0x10000 != 0 {
        sqrt_price = sqrt_price * PRICE_0X10000;
    }
    if abs_tick & 0x20000 != 0 {
        sqrt_price = sqrt_price * PRICE_0X20000;
    }
    if abs_tick & 0x40000 != 0 {
        sqrt_price = sqrt_price * PRICE_0X40000;
    }
    if abs_tick & 0x80000 != 0 {
        sqrt_price = sqrt_price * PRICE_0X80000;
    }

    if tick < 0 {
        sqrt_price = Decimal::one() / sqrt_price;
    }
    sqrt_price
}

/**
 * We use the same property used to compute sqrt_price_at_tick, to compute tick_at_sqrt_price, but instead of multiplying, now we
 * are dividing the given price by the pre-computed constants, in the same time adding the corresponding exponent to the target tick.
 * 
 * In the end we want to make sure the sqrt_price_at_tick and tick_at_sqrt_price return consistent values, avoiding rounding errors.
 */
pub fn tick_at_sqrt_price(sqrt_price_in: Decimal) -> i32 {
    assert!(sqrt_price_in >= MIN_PRICE && sqrt_price_in <= MAX_PRICE, "Sqrt price out of bounds.");

    let is_negative_tick = sqrt_price_in < Decimal::one();
    let mut sqrt_price = if is_negative_tick { Decimal::one() / sqrt_price_in } else {sqrt_price_in};
    let mut tick = 0;
    if sqrt_price >= PRICE_0X80000 {
        sqrt_price = sqrt_price / PRICE_0X80000;
        tick += 0x80000; //2^19
    } 
    if sqrt_price >= PRICE_0X40000 {
        sqrt_price = sqrt_price / PRICE_0X40000;
        tick += 0x40000 //2^18
    }
    if sqrt_price >= PRICE_0X20000 {
        sqrt_price = sqrt_price / PRICE_0X20000;
        tick += 0x20000; //2^17
    }
    if sqrt_price >= PRICE_0X10000 {
        sqrt_price = sqrt_price / PRICE_0X10000;
        tick += 0x10000; //2^16
    }
    if sqrt_price >= PRICE_0X8000 {
        sqrt_price = sqrt_price / PRICE_0X8000;
        tick += 0x8000; //2^15
    }
    if sqrt_price >= PRICE_0X4000 {
        sqrt_price = sqrt_price / PRICE_0X4000;
        tick += 0x4000; //2^14
    }
    if sqrt_price >= PRICE_0X2000 {
        sqrt_price = sqrt_price / PRICE_0X2000;
        tick += 0x2000; //2^13
    }
    if sqrt_price >= PRICE_0X1000 {
        sqrt_price = sqrt_price / PRICE_0X1000;
        tick += 0x1000; //2^12
    }
    if sqrt_price >= PRICE_0X800 {
        sqrt_price = sqrt_price / PRICE_0X800;
        tick += 0x800; //2^11
    }
    if sqrt_price >= PRICE_0X400 {
        sqrt_price = sqrt_price / PRICE_0X400;
        tick += 0x400; //2^10
    }
    if sqrt_price >= PRICE_0X200 {
        sqrt_price = sqrt_price / PRICE_0X200;
        tick += 0x200; //2^9
    }
    if sqrt_price >= PRICE_0X100 {
        sqrt_price = sqrt_price / PRICE_0X100;
        tick += 0x100; //2^8
    }
    if sqrt_price >= PRICE_0X80 {
        sqrt_price = sqrt_price / PRICE_0X80;
        tick += 0x80; //2^7
    }
    if sqrt_price >= PRICE_0X40 {
        sqrt_price = sqrt_price / PRICE_0X40;
        tick += 0x40; //2^6
    }
    if sqrt_price >= PRICE_0X20 {
        sqrt_price = sqrt_price / PRICE_0X20;
        tick += 0x20; //2^5
    }
    if sqrt_price >= PRICE_0X10 {
        sqrt_price = sqrt_price / PRICE_0X10;
        tick += 0x10; //2^4
    }
    if sqrt_price >= PRICE_0X8 {
        sqrt_price = sqrt_price / PRICE_0X8;
        tick += 0x8; //2^3
    }
    if sqrt_price >= PRICE_0X4 {
        sqrt_price = sqrt_price / PRICE_0X4;
        tick += 0x4; //2^2
    }
    if sqrt_price >= PRICE_0X2 {
        sqrt_price = sqrt_price / PRICE_0X2;
        tick += 0x2; //2^1
    }
    if sqrt_price >= PRICE_0X1 {
        tick += 0x1; //2^0
    }

    let tick_candidate = if is_negative_tick { -tick } else { tick };
    let sqrt_price_tick_candidate = sqrt_price_at_tick(tick_candidate);
    if sqrt_price_tick_candidate == sqrt_price_in { tick_candidate }
    else if sqrt_price_tick_candidate > sqrt_price_in { tick_candidate - 1 }
    else { tick_candidate + 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqrt_price_at_tick() {
        assert_eq!(MIN_PRICE, sqrt_price_at_tick(MIN_TICK));
        assert_eq!(dec!("0.000000000276890319"), sqrt_price_at_tick(-440170));
        assert_eq!(dec!("0.999850018747812746"), sqrt_price_at_tick(-3));
        assert_eq!(dec!("0.999950003749687527"), sqrt_price_at_tick(-1));
        assert_eq!(Decimal::one(), sqrt_price_at_tick(0));
        assert_eq!(dec!("1.000150003749937502"), sqrt_price_at_tick(3));
        assert_eq!(dec!("1.000250018750312495"), sqrt_price_at_tick(5));
        assert_eq!(dec!("1.451912069310684182"), sqrt_price_at_tick(7458));
        assert_eq!(dec!("13.043260825728760908"), sqrt_price_at_tick(51368));
        assert_eq!(dec!("3611718901.096063128233128884"), sqrt_price_at_tick(440171));
        assert_eq!(MAX_PRICE, sqrt_price_at_tick(MAX_TICK));
    }

    #[test]
    fn test_tick_at_sqrt_price() {
        assert_eq!(0, tick_at_sqrt_price(Decimal::one()));
        assert_eq!(1, tick_at_sqrt_price(dec!("1.0000499987500624")));
        assert_eq!(2, tick_at_sqrt_price( dec!("1.0001")));
        assert_eq!(3, tick_at_sqrt_price(dec!("1.000150003749937406")));
        assert_eq!(-3, tick_at_sqrt_price(dec!("0.999850018747812842")));
        assert_eq!(-440170, tick_at_sqrt_price(dec!("0.000000000276890319")));
        assert_eq!(-440170, tick_at_sqrt_price(dec!("0.00000000027689032")));
        assert_eq!(440171, tick_at_sqrt_price(dec!("3611718901.08879675118568791")));
        assert_eq!(440171, tick_at_sqrt_price(dec!("3611718901.08879675118568792")));
        assert_eq!(MAX_TICK, tick_at_sqrt_price(MAX_PRICE));
        assert_eq!(MIN_TICK, tick_at_sqrt_price(MIN_PRICE));
    }

    /**
     * Tests that we are consistent with all the ticks between MIN_TICK and MAX_TICK. Depending on the machine, this test can take 1-2 minutes to run, so by default is disabled.
     */
    #[test]
    #[ignore]
    fn tick_at_sqrt_price_equals_original_tick() {
        for tick in (MIN_TICK..MAX_TICK + 1).rev() {
            assert_eq!(tick, tick_at_sqrt_price(sqrt_price_at_tick(tick)));
        }
    }

    #[test]
    fn test_x() {
        let a = bnum_integer::I192::from_str("170134484377190040957155711420855095752").unwrap();

        println!("{:?}", a.0.to_bits().digits());
        
        let x = Decimal(a);
        let y = dec!("170134484377190040957.155711420855095752");
        let z = Decimal(bnum_integer::I192::from_digits([4809668506064654792, 9223008878822527810, 0]));
        assert_eq!(x, y);
        assert_eq!(y, z);
    }
}
