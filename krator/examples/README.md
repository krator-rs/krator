# Moose Example

This is a small sample application using most of the features `krator` and
`krator_derive` have to offer.

## Run example

In order to use all features from the moose example, compile it with
`admission-webhook`, `derive` and `krator-derive/admission-webhook` features
enabled -- this is reflected through the aggregate feature
`derive-admission-webhook`

### Run without webhook

Install crd

```console
RUST_LOG=moose=info,krator=info cargo run --example=moose --features=derive-admission-webhook -- --output-crd | kubectl apply -f-
```

Run operator

```console
# w/o admission webhook
RUST_LOG=moose=info,krator=info cargo run --example=moose --features=derive 
```

### Run with webhook

Install crd and webhook resources into a namespace

```console
NAMESPACE=default
RUST_LOG=moose=info,krator=info cargo run --example=moose --features=derive-admission-webhook -- --output-crd | kubectl apply -f-
RUST_LOG=moose=info,krator=info cargo run --example=moose --features=derive-admission-webhook -- --output-webhook-resources-for-namespace $NAMESPACE | kubectl apply -f-
```

Run operator and follow the instructions that are printed

```console
RUST_LOG=moose=info,krator=info cargo run --example=moose --features=derive-admission-webhook
```
