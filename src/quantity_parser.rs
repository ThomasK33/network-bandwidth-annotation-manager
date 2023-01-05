use color_eyre::Result;
use regex::Regex;

/// Parses a Kubernetes `Quantity` as defined in: https://github.com/kubernetes/apimachinery/blob/master/pkg/api/resource/quantity.go#L31
///
/// The serialization format is:
///
/// ```
/// <quantity>        ::= <signedNumber><suffix>
///
/// (Note that <suffix> may be empty, from the "" case in <decimalSI>.)
///
/// <digit>           ::= 0 | 1 | ... | 9
/// <digits>          ::= <digit> | <digit><digits>
/// <number>          ::= <digits> | <digits>.<digits> | <digits>. | .<digits>
/// <sign>            ::= "+" | "-"
/// <signedNumber>    ::= <number> | <sign><number>
/// <suffix>          ::= <binarySI> | <decimalExponent> | <decimalSI>
/// <binarySI>        ::= Ki | Mi | Gi | Ti | Pi | Ei
///
/// (International System of units; See: http://physics.nist.gov/cuu/Units/binary.html)
///
/// <decimalSI>       ::= m | "" | k | M | G | T | P | E
///
/// (Note that 1024 = 1Ki but 1000 = 1k; I didn't choose the capitalization.)
///
/// <decimalExponent> ::= "e" <signedNumber> | "E" <signedNumber>
/// ```
pub fn parse(quantity: &str) -> Result<f64> {
    let regex = Regex::new(r"([[:alpha:]]{1,2}$)")?;

    let Some(suffix) = regex.captures(quantity) else {
        return Ok(quantity.parse()?);
    };

    let Some(suffix) = suffix.get(0) else {
        return Err(color_eyre::eyre::eyre!("Could not determine quantity suffix"));
    };

    let multiplier: f64 = match suffix.as_str() {
        "Ki" => 1024.0,
        "Mi" => 1024_f64.powi(2),
        "Gi" => 1024_f64.powi(3),
        "Ti" => 1024_f64.powi(4),
        "Pi" => 1024_f64.powi(5),
        "Ei" => 1024_f64.powi(6),
        "k" => 1000.0,
        "M" => 1000_f64.powi(2),
        "G" => 1000_f64.powi(3),
        "T" => 1000_f64.powi(4),
        "P" => 1000_f64.powi(5),
        "E" => 1000_f64.powi(6),
        "m" => 1.0 / 1000.0,
        _ => {
            return Err(color_eyre::eyre::eyre!(
                "Unknown quantity suffix used: {suffix:?}"
            ));
        }
    };

    let quantity = quantity.replace(suffix.as_str(), "");
    let quantity = quantity.parse::<f64>()?;

    Ok(quantity * multiplier)
}
