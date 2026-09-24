//! +-----------------------------------------------------------------+
//! | eo-rt — runtime support library for compiled EO programs.       |
//! |                                                                  |
//! | Provides the adaptive `Value` representation, the `Object` /     |
//! | `Class` records that back the slow path, and the public          |
//! | `dispatch` entry point that compiled code emits calls to.        |
//! |                                                                  |
//! | The fast path keeps values in 64-bit registers (small ints       |
//! | tagged in the low bit). The slow path falls through to a real    |
//! | `&'static Object` whose `Class` knows how to handle every attr   |
//! | the EO source named.                                             |
//! |                                                                  |
//! | This crate intentionally has no allocations on the fast path     |
//! | and no `HashMap`s anywhere — attr names are interned to numeric  |
//! | ids by `eo-compile` at build time.                                |
//! +-----------------------------------------------------------------+

#![forbid(unsafe_op_in_unsafe_fn)]

/// Numeric identifier for an attribute name.
///
/// `eo-compile` assigns a stable id to every attribute name observed
/// while lowering the EO program. Compiled code uses these ids in
/// every dispatch call site so the runtime never compares strings.
pub type AttrId = u32;

pub const ATTR_PLUS: AttrId = 1;
pub const ATTR_MINUS: AttrId = 2;
pub const ATTR_TIMES: AttrId = 3;
pub const ATTR_GT: AttrId = 4;
pub const ATTR_LT: AttrId = 5;
pub const ATTR_EQ: AttrId = 6;
pub const ATTR_IF: AttrId = 7;
pub const ATTR_PHI: AttrId = 8;
pub const ATTR_RHO: AttrId = 9;

/// Adaptive value: either a tagged small int (low bit = 1)
/// or a pointer to a real `Object` (low bit = 0).
///
/// Layout (bit 0 = tag):
///
/// ```text
/// small int :  | i63 payload                                | 1 |
/// object ptr:  | aligned &'static Object pointer            | 0 |
/// ```
///
/// `Object`s are pointer-aligned, so the bottom three bits are
/// available; the runtime currently uses only bit 0. The high
/// 63 bits of a small int are sign-extended on access.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Value(pub u64);

impl Value {
    pub const fn small_int(n: i64) -> Self {
        Self(((n as u64) << 1) | 1)
    }

    pub const fn is_small_int(self) -> bool {
        (self.0 & 1) == 1
    }

    pub const fn as_small_int(self) -> i64 {
        (self.0 as i64) >> 1
    }

    pub fn from_object(obj: &'static Object) -> Self {
        Self(obj as *const Object as u64)
    }

    pub fn as_object(self) -> Option<&'static Object> {
        if self.is_small_int() {
            None
        } else {
            unsafe { Some(&*(self.0 as *const Object)) }
        }
    }
}

/// +-----------------------------------------------------------------+
/// | Class — describes a shape (slot layout) and a dispatch fn.       |
/// |                                                                  |
/// | Every real `Object` carries a `&'static Class`. The dispatch fn  |
/// | receives the receiver, the attr id, and the bound arguments,    |
/// | and returns the resulting `Value`. Specialized arithmetic        |
/// | classes (see `SMALL_INTEGER`) keep the result in tagged form.    |
/// +-----------------------------------------------------------------+
pub struct Class {
    pub name: &'static str,
    pub dispatch: fn(receiver: Value, attr: AttrId, args: &[Value]) -> Value,
}

/// +-----------------------------------------------------------------+
/// | Object — heap-allocated representation of an EO formation.       |
/// |                                                                  |
/// | Used on the slow path. The `class` pointer drives all dispatch.  |
/// | Slot storage is appended by the codegen and read by the class'   |
/// | dispatch fn at known offsets (hidden-class style).               |
/// +-----------------------------------------------------------------+
#[repr(C)]
pub struct Object {
    pub class: &'static Class,
}

/// Dispatch primitive emitted by compiled code.
///
/// Branches on the tag once, then either runs the small-int fast path
/// inline or hands off to the class' dispatch fn. The branch is
/// expected to be highly predictable per call site, so inline-cache
/// elimination by the host compiler is straightforward.
#[inline]
pub fn dispatch(recv: Value, attr: AttrId, args: &[Value]) -> Value {
    if recv.is_small_int() {
        small_integer_dispatch(recv, attr, args)
    } else {
        let obj = recv.as_object().expect("dispatch: receiver is neither small int nor object");
        (obj.class.dispatch)(recv, attr, args)
    }
}

/// Static class for tagged small integers.
///
/// Used by `inflate_small_int` when a small int has to escape into
/// the slow path (e.g., stored where a `&'static Object` is required).
pub static SMALL_INTEGER: Class = Class {
    name: "small-integer",
    dispatch: small_integer_dispatch,
};

fn small_integer_dispatch(recv: Value, attr: AttrId, args: &[Value]) -> Value {
    let lhs = recv.as_small_int();
    match attr {
        ATTR_PLUS => Value::small_int(lhs.wrapping_add(args[0].as_small_int())),
        ATTR_MINUS => Value::small_int(lhs.wrapping_sub(args[0].as_small_int())),
        ATTR_TIMES => Value::small_int(lhs.wrapping_mul(args[0].as_small_int())),
        ATTR_GT => Value::small_int(if lhs > args[0].as_small_int() { 1 } else { 0 }),
        ATTR_LT => Value::small_int(if lhs < args[0].as_small_int() { 1 } else { 0 }),
        ATTR_EQ => Value::small_int(if lhs == args[0].as_small_int() { 1 } else { 0 }),
        _ => panic!("small_integer_dispatch: unsupported attr id {attr}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_int_round_trips_through_value() {
        let v = Value::small_int(42);
        assert!(v.is_small_int(), "small int must report is_small_int");
        assert_eq!(v.as_small_int(), 42);
    }

    #[test]
    fn small_int_round_trips_negative() {
        let v = Value::small_int(-7);
        assert_eq!(v.as_small_int(), -7);
    }

    #[test]
    fn small_int_plus_stays_tagged() {
        let r = dispatch(Value::small_int(3), ATTR_PLUS, &[Value::small_int(4)]);
        assert!(r.is_small_int(), "tagged + tagged must stay tagged");
        assert_eq!(r.as_small_int(), 7);
    }

    #[test]
    fn small_int_lt_returns_one_for_true() {
        let r = dispatch(Value::small_int(1), ATTR_LT, &[Value::small_int(2)]);
        assert_eq!(r.as_small_int(), 1);
    }

    #[test]
    fn small_int_lt_returns_zero_for_false() {
        let r = dispatch(Value::small_int(5), ATTR_LT, &[Value::small_int(2)]);
        assert_eq!(r.as_small_int(), 0);
    }
}
