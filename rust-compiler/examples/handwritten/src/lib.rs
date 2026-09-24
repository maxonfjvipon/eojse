//! +-----------------------------------------------------------------+
//! | handwritten — what `eo-compile` should emit for each EO program.|
//! |                                                                  |
//! | One fn per `test-resources/*.yaml`. Each fn is the post-         |
//! | inference, post-lowering shape: small ints in tagged form, all  |
//! | dispatches that survive go through `eo_rt::dispatch`, recursion |
//! | uses native call frames. No allocations on the hot path.        |
//! +-----------------------------------------------------------------+

use eo_rt::{dispatch, Value, ATTR_GT, ATTR_LT, ATTR_MINUS, ATTR_PLUS};

pub fn simple() -> Value {
    Value::small_int(42)
}

pub fn foo() -> Value {
    fn identity(y: Value) -> Value {
        y
    }
    identity(identity(Value::small_int(35)))
}

pub fn eleven() -> Value {
    dispatch(Value::small_int(5), ATTR_PLUS, &[Value::small_int(6)])
}

pub fn rec() -> Value {
    fn body(x: Value) -> Value {
        let cond = dispatch(Value::small_int(5), ATTR_GT, &[x]);
        if cond.as_small_int() != 0 {
            body(dispatch(x, ATTR_PLUS, &[Value::small_int(1)]))
        } else {
            x
        }
    }
    body(Value::small_int(1))
}

pub fn dollar() -> Value {
    fn foo_get_x(x: Value) -> Value {
        x
    }
    foo_get_x(Value::small_int(42))
}

pub fn dup() -> Value {
    dispatch(Value::small_int(1), ATTR_PLUS, &[Value::small_int(1)])
}

pub fn self_ref() -> Value {
    fn body(x: Value) -> Value {
        let next = dispatch(x, ATTR_PLUS, &[Value::small_int(1)]);
        let cond = dispatch(Value::small_int(5), ATTR_GT, &[x]);
        if cond.as_small_int() != 0 {
            body(next)
        } else {
            x
        }
    }
    body(Value::small_int(1))
}

pub fn fibo(n: Value) -> Value {
    let cond = dispatch(n, ATTR_LT, &[Value::small_int(2)]);
    if cond.as_small_int() != 0 {
        n
    } else {
        let a = fibo(dispatch(n, ATTR_MINUS, &[Value::small_int(1)]));
        let b = fibo(dispatch(n, ATTR_MINUS, &[Value::small_int(2)]));
        dispatch(a, ATTR_PLUS, &[b])
    }
}

pub fn fibo_default() -> Value {
    fibo(Value::small_int(8))
}

pub fn fibo_minus() -> Value {
    fibo(Value::small_int(8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_returns_42() {
        assert_eq!(simple().as_small_int(), 42, "simple must yield literal 42");
    }

    #[test]
    fn foo_returns_35() {
        assert_eq!(foo().as_small_int(), 35, "foo of foo of 35 must be 35");
    }

    #[test]
    fn eleven_returns_11() {
        assert_eq!(eleven().as_small_int(), 11, "5.plus 6 must be 11");
    }

    #[test]
    fn rec_returns_5() {
        assert_eq!(rec().as_small_int(), 5, "rec must stop at 5");
    }

    #[test]
    fn dollar_returns_42() {
        assert_eq!(dollar().as_small_int(), 42, "self.x in foo 42 must be 42");
    }

    #[test]
    fn dup_returns_2() {
        assert_eq!(dup().as_small_int(), 2, "1.plus 1 must be 2");
    }

    #[test]
    fn self_ref_returns_5() {
        assert_eq!(self_ref().as_small_int(), 5, "self-ref must stop at 5");
    }

    #[test]
    fn fibo_default_returns_21() {
        assert_eq!(fibo_default().as_small_int(), 21, "fibo(8) must be 21");
    }

    #[test]
    fn fibo_minus_returns_21() {
        assert_eq!(fibo_minus().as_small_int(), 21, "fibo_minus(8) must be 21");
    }

    #[test]
    fn fibo_grows_through_28() {
        let cases = [
            (8i64, 21i64),
            (9, 34),
            (10, 55),
            (11, 89),
            (12, 144),
            (15, 610),
            (20, 6765),
            (25, 75025),
            (28, 317811),
        ];
        for (n, expected) in cases {
            let got = fibo(Value::small_int(n)).as_small_int();
            assert_eq!(got, expected, "fibo({n}) must be {expected}, got {got}");
        }
    }
}
