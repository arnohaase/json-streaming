
/// This trait allows customization of how json-streaming formats floating point numbers.
///
/// There are many valid string representations of a given number. The number `10.0` can e.g.
///  be represented as `10.0`, `10`, `1e2`, `1.0e2`, or `1.0e+2` and many others. There is no
///  technical reason to customize this, it is entirely about human readability.
///
/// See the 'float_format' example for details.
pub trait FloatFormat {
    fn write_f64(f: &mut impl core::fmt::Write, value: f64) -> core::fmt::Result;
    fn write_f32(f: &mut impl core::fmt::Write, value: f32) -> core::fmt::Result;
}

/// This is the default formatter for floating point numbers. It writes numbers from 1e-3 to
///  1e6 as regular decimal numbers, and numbers outside that range in exponential representation.
pub struct DefaultFloatFormat;
impl FloatFormat for DefaultFloatFormat {
    fn write_f64(f: &mut impl core::fmt::Write, value: f64) -> core::fmt::Result {
        const UPPER_BOUND_LIT:f64 = 1e6;
        const LOWER_BOUND_LIT:f64 = 1e-3;

        if value.is_finite() {
            if value.abs() < UPPER_BOUND_LIT && value.abs() >= LOWER_BOUND_LIT {
                write!(f, "{}", value)
            }
            else {
                write!(f, "{:e}", value)
            }
        }
        else {
            write!(f, "null")
        }
    }

    fn write_f32(f: &mut impl core::fmt::Write, value: f32) -> core::fmt::Result {
        const UPPER_BOUND_LIT:f32 = 1e6;
        const LOWER_BOUND_LIT:f32 = 1e-3;

        if value.is_finite() {
            if value.abs() < UPPER_BOUND_LIT && value.abs() >= LOWER_BOUND_LIT {
                write!(f, "{}", value)
            }
            else {
                write!(f, "{:e}", value)
            }
        }
        else {
            write!(f, "null")
        }
    }
}
