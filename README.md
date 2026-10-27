# Shopify First Month Free Discount Function Starter

**Lead Developer:** Nícolas Cartín Reyes
**License:** MIT

A production-oriented starter for a Shopify app extension that gives one configured membership line a 100 percent product discount when the cart also contains an eligible equipment line. The starter uses a Shopify Discount Function compiled from Rust to WebAssembly and a discount-owned JSON metafield for configuration.

This repository is intentionally store-neutral. It contains no merchant name, store domain, app client ID, product IDs, variant IDs, access tokens, checkout data, customer data, or production validation records.

## What the starter does

At cart and checkout evaluation time, Shopify invokes the `cart.lines.discounts.generate.run` target. The Function:

1. Requires the Product discount class.
2. Reads `membershipProductIds` and `membershipVariantIds` from the discount-owned JSON metafield.
3. Finds the first cart line whose variant ID is in `membershipVariantIds`.
4. Finds a separate product variant line whose parent product has a membership-offer metafield containing one of the configured membership product IDs.
5. Applies a 100 percent product discount to quantity one of the membership line.
6. Returns no operation when configuration, the membership line, or the eligible equipment prerequisite is missing.

The equipment product name, product tags, customer identity, payment data, and shipping rates are not used by the Function. The equipment eligibility relationship is carried by the product metafield requested in `src/cart_lines_discounts_generate_run.graphql`.

## Architecture

This starter is Shopify-native and does not require a hosted server for checkout execution.

- `extensions/membership-first-month-free/src/cart_lines_discounts_generate_run.rs` contains the Rust Function.
- `extensions/membership-first-month-free/src/cart_lines_discounts_generate_run.graphql` defines the narrow Shopify input query.
- `extensions/membership-first-month-free/shopify.extension.toml` maps the extension to the Product Discount target.
- `extensions/membership-first-month-free/src/main.rs` enables Shopify Function type generation.
- `extensions/membership-first-month-free/schema.graphql` is the generated Shopify Function schema used by type generation.
- `shopify.app.toml.example` is a sanitized app configuration template. Copy it to `shopify.app.toml` only after linking the project to an app that you own.

The starter does not include an App Home or Admin API control plane. A production app can add one to create and manage the automatic discount through an authenticated Shopify Admin API context. Creating the source Function and creating the live Shopify discount are separate operations.

## Configuration contract

The automatic discount must own a JSON metafield with this shape:

```json
{
  "membershipProductIds": [
    "gid://shopify/Product/EXAMPLE_MEMBERSHIP_PRODUCT_ID"
  ],
  "membershipVariantIds": [
    "gid://shopify/ProductVariant/EXAMPLE_MEMBERSHIP_VARIANT_ID"
  ]
}
```

Use real GIDs only in the Shopify store where the app is installed. Keep those values in store-side configuration or a private deployment repository. Do not commit merchant-specific IDs to a public repository.

The equipment product metafield used by the starter is:

```text
namespace: custom
key: membership_offers
value: JSON array of membership product GIDs
```

For example:

```json
["gid://shopify/Product/EXAMPLE_MEMBERSHIP_PRODUCT_ID"]
```

The metafield definition and the automatic discount record must exist in the target store before the Function can apply a discount. The source code alone does not create either record.

## Local setup

Prerequisites:

- Shopify CLI
- Rust stable
- Rust target `wasm32-unknown-unknown`
- A Shopify development organization and an app that you are authorized to manage
- A development store that supports the selected Shopify Function capability

```bash
cp shopify.app.toml.example shopify.app.toml
shopify app config link
rustup target add wasm32-unknown-unknown
cargo fmt --manifest-path extensions/membership-first-month-free/Cargo.toml --check
cargo test --manifest-path extensions/membership-first-month-free/Cargo.toml
shopify app build
```

Review the build output and extension mapping before deploying. A new extension UID or handle can create a new Function registration instead of updating the intended one.

## Store-side activation

After the Function is deployed and registered, create an automatic app discount that points to the Function handle `membership-first-month-free`. The discount should use the Product discount class and store the JSON configuration described above.

A generic Admin GraphQL shape is:

```graphql
mutation CreateAutomaticDiscount($input: DiscountAutomaticAppInput!) {
  discountAutomaticAppCreate(automaticAppDiscount: $input) {
    automaticAppDiscount {
      discountId
      title
      status
      startsAt
      endsAt
      appDiscountType { functionId }
    }
    userErrors { field code message }
  }
}
```

For a permanent discount, omit `endsAt` or explicitly set it to `null`. Choose `combinesWith` values as a business decision and test them in the target store. A Product Discount Function can be independent from an order discount while still being configured to combine with order, product, or shipping discounts.

Do not copy the mutation variables from this starter into a production store without replacing the example title, GIDs, dates, and Function ownership details.

## Testing matrix

At minimum, verify all of the following in a development or controlled store:

- Eligible equipment plus the configured membership produces a 100 percent discount on one membership unit.
- The equipment line remains at its normal price.
- Membership without eligible equipment produces no Product Discount operation.
- Equipment without the membership-offer metafield produces no Product Discount operation.
- An excluded membership variant produces no operation.
- Multiple membership units discount only quantity one.
- Missing or malformed JSON configuration fails closed.
- The discount class is Product.
- The live discount points to the Function registered by the current app version.
- Cart and checkout show the expected savings without submitting payment or creating an order.

## Safe deployment workflow

Use a branch and pull request for source changes. Before deployment:

1. Inspect `git diff` and confirm that no store IDs, customer data, access tokens, or environment files are included.
2. Run Rust formatting, unit tests, and `shopify app build`.
3. Run `shopify app info` and confirm the intended app and organization.
4. Run `shopify app deploy` and inspect the extension mapping and release prompt.
5. Confirm the Function handle and UID remain mapped to the intended extension.
6. Configure or update the store-side discount through an authorized Admin API context.
7. Validate a controlled cart and checkout without payment or order submission.
8. Record the app version and rollback reference privately.

Rollback has two separate surfaces. Releasing an earlier app version changes the Function implementation. Deactivating or deleting the automatic discount changes store-side activation. Treat both as reviewed operations.

## Security boundary

Never commit Shopify passwords, Partner credentials, client secrets, Admin API access tokens, session cookies, browser profiles, `.env` files, checkout URLs, customer data, or private store identifiers. Public source can explain the architecture and configuration contract, but live deployment requires authenticated access to the owning Shopify app and store.

## References

- [Shopify Functions](https://shopify.dev/docs/apps/build/functions)
- [Shopify CLI for apps](https://shopify.dev/docs/apps/build/cli-for-apps)
- [Discount Function input and output concepts](https://shopify.dev/docs/apps/build/discounts)
- [Shopify Admin GraphQL automatic app discounts](https://shopify.dev/docs/api/admin-graphql/latest/mutations/discountAutomaticAppCreate)
