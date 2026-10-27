use crate::schema;
use crate::schema::cart_lines_discounts_generate_run::input::cart::lines::Merchandise;
use shopify_function::prelude::*;
use shopify_function::scalars::JsonValue;
use shopify_function::Result;

pub const DISCOUNT_MESSAGE: &str = "First Month Free";

/// Configuration stored on the automatic discount in the JSON metafield
/// `starter.membership-first-month-free-config`:
///
/// {
///   "membershipProductIds": ["gid://shopify/Product/1001"],
///   "membershipVariantIds": ["gid://shopify/ProductVariant/2001"]
/// }
///
/// `membershipProductIds` identifies the configured membership products offered by eligible
/// equipment PDPs. The Function reads each equipment Product's
/// `custom.membership_offers` metafield at checkout evaluation time.
/// `membershipVariantIds` identifies the exact configured membership cart variants
/// that can receive the free first-month discount.
#[derive(Default)]
pub struct Configuration {
    pub membership_product_ids: Vec<String>,
    pub membership_variant_ids: Vec<String>,
}

impl Configuration {
    pub fn from_json(value: Option<&JsonValue>) -> Self {
        let Some(JsonValue::Object(object)) = value else {
            return Self::default();
        };

        fn ids(object: &std::collections::BTreeMap<String, JsonValue>, key: &str) -> Vec<String> {
            match object.get(key) {
                Some(JsonValue::Array(values)) => values
                    .iter()
                    .filter_map(|value| match value {
                        JsonValue::String(id) if id.starts_with("gid://shopify/") => {
                            Some(id.clone())
                        }
                        _ => None,
                    })
                    .collect(),
                _ => vec![],
            }
        }

        Self {
            membership_product_ids: ids(object, "membershipProductIds"),
            membership_variant_ids: ids(object, "membershipVariantIds"),
        }
    }
}

/// `custom.membership_offers` is a list.product_reference metafield, so
/// its value is a JSON array string such as
/// `["gid://shopify/Product/1","gid://shopify/Product/2"]`.
/// Match the quoted GID exactly so `Product/1` never matches `Product/12`.
pub fn offers_membership(metafield_value: &str, membership_product_ids: &[String]) -> bool {
    membership_product_ids
        .iter()
        .any(|id| metafield_value.contains(&format!("\"{}\"", id)))
}

pub fn membership_variant_is_eligible(variant_id: &str, membership_variant_ids: &[String]) -> bool {
    membership_variant_ids.iter().any(|id| id == variant_id)
}

#[shopify_function]
fn cart_lines_discounts_generate_run(
    input: schema::cart_lines_discounts_generate_run::Input,
) -> Result<schema::CartLinesDiscountsGenerateRunResult> {
    let empty = || Ok(schema::CartLinesDiscountsGenerateRunResult { operations: vec![] });

    if !input
        .discount()
        .discount_classes()
        .contains(&schema::DiscountClass::Product)
    {
        return empty();
    }

    let configuration = Configuration::from_json(
        input
            .discount()
            .metafield()
            .map(|metafield| metafield.json_value()),
    );

    if configuration.membership_product_ids.is_empty()
        || configuration.membership_variant_ids.is_empty()
    {
        return empty();
    }

    let mut equipment_line_id: Option<String> = None;
    let mut membership_line_id: Option<String> = None;

    for line in input.cart().lines() {
        let Merchandise::ProductVariant(variant) = line.merchandise() else {
            continue;
        };

        let variant_id = variant.id().to_string();
        if membership_variant_is_eligible(&variant_id, &configuration.membership_variant_ids) {
            if membership_line_id.is_none() {
                membership_line_id = Some(line.id().to_string());
            }
            continue;
        }

        if equipment_line_id.is_none() {
            let offers = variant
                .product()
                .membership_offer()
                .map(|metafield| {
                    offers_membership(metafield.value(), &configuration.membership_product_ids)
                })
                .unwrap_or(false);

            if offers {
                equipment_line_id = Some(line.id().to_string());
            }
        }
    }

    let (Some(equipment_line_id), Some(membership_line_id)) =
        (equipment_line_id, membership_line_id)
    else {
        return empty();
    };

    Ok(schema::CartLinesDiscountsGenerateRunResult {
        operations: vec![schema::CartOperation::ProductDiscountsAdd(
            schema::ProductDiscountsAddOperation {
                selection_strategy: schema::ProductDiscountSelectionStrategy::First,
                candidates: vec![schema::ProductDiscountCandidate {
                    targets: vec![schema::ProductDiscountCandidateTarget::CartLine(
                        schema::CartLineTarget {
                            id: membership_line_id,
                            quantity: Some(1),
                        },
                    )],
                    message: Some(DISCOUNT_MESSAGE.to_string()),
                    value: schema::ProductDiscountCandidateValue::Percentage(schema::Percentage {
                        value: Decimal(100.0),
                    }),
                    associated_discount_code: None,
                    prerequisites: Some(vec![
                        schema::ProductDiscountCandidatePrerequisite::CartLine(
                            schema::CartLinePrerequisite {
                                id: equipment_line_id,
                                quantity: 1,
                            },
                        ),
                    ]),
                }],
            },
        )],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configuration(products: &[&str], variants: &[&str]) -> Configuration {
        Configuration {
            membership_product_ids: products.iter().map(|id| id.to_string()).collect(),
            membership_variant_ids: variants.iter().map(|id| id.to_string()).collect(),
        }
    }

    #[test]
    fn metafield_match_is_exact() {
        let ids = vec!["gid://shopify/Product/1001".to_string()];
        assert!(offers_membership(
            r#"["gid://shopify/Product/1001","gid://shopify/Product/1003"]"#,
            &ids
        ));
        assert!(!offers_membership(
            r#"["gid://shopify/Product/10011"]"#,
            &ids
        ));
        assert!(!offers_membership(r#"[]"#, &ids));
    }

    #[test]
    fn only_configured_membership_variant_is_discountable() {
        let config = configuration(
            &["gid://shopify/Product/1001"],
            &["gid://shopify/ProductVariant/2001"],
        );
        assert!(membership_variant_is_eligible(
            "gid://shopify/ProductVariant/2001",
            &config.membership_variant_ids
        ));
        assert!(!membership_variant_is_eligible(
            "gid://shopify/ProductVariant/2002",
            &config.membership_variant_ids
        ));
    }

    #[test]
    fn equipment_must_offer_configured_membership_on_its_pdp() {
        let config = configuration(&["gid://shopify/Product/1001"], &[]);
        assert!(offers_membership(
            r#"["gid://shopify/Product/1001","gid://shopify/Product/1002"]"#,
            &config.membership_product_ids
        ));
        assert!(!offers_membership(
            r#"["gid://shopify/Product/1002"]"#,
            &config.membership_product_ids
        ));
    }

    #[test]
    fn missing_configuration_fails_closed() {
        let config = Configuration::from_json(None);
        assert!(config.membership_product_ids.is_empty());
        assert!(config.membership_variant_ids.is_empty());
    }
}
