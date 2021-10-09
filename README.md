# Krator: Kubernetes Operators using State Machines

[![CII Best Practices](https://bestpractices.coreinfrastructure.org/projects/5066/badge)](https://bestpractices.coreinfrastructure.org/projects/5066)

:construction: :construction: **This project is highly experimental.**
:construction: :construction: It should not be used in production workloads.

Krator acts as an Operator by watching Kubernetes resources and running
control loops to reconcile cluster state with desired state. Control loops are
specified using a State Machine API pattern which improves reliability and
reduces complexity.

## Documentation

[API Documentation](https://docs.rs/krator)

Looking for the developer guide? [Start here](docs/community/developers.md).

## Examples

[Moose Operator](krator/examples)

## Community, discussion, contribution, and support

You can reach the Krator community and developers via the following channels:

- [Kubernetes Slack](https://kubernetes.slack.com):
  - [#krator](https://kubernetes.slack.com/messages/krator)

## Code of Conduct

This project has adopted the [Microsoft Open Source Code of
Conduct](https://opensource.microsoft.com/codeofconduct/).

For more information see the [Code of Conduct
FAQ](https://opensource.microsoft.com/codeofconduct/faq/) or contact
[opencode@microsoft.com](mailto:opencode@microsoft.com) with any additional
questions or comments.

## Vulnerability Reporting

For sensitive issues, please email one of the project maintainers. For
other issues, please open an issue in this GitHub repository.
