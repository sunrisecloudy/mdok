# mdok CLI adapter boundary

`mdok-runtime` is currently a skeleton and does not expose a usable planning or
execution API. The CLI therefore keeps the integration behind the local
`DocumentPlan`/`StepPlan` adapter in `src/main.rs`:

* fence discovery, TOML variables, template rendering, policy validation, and
  JMESPath compilation are performed before any transfer;
* `test` uses the same plan and performs basic blocking HTTP through reqwest;
* report types and output formats are supplied by `mdok-report` and do not
  depend on the adapter's internal representation.

When runtime APIs become available, the adapter can be replaced at
`build_plan`/`execute_plan` without changing clap options, exit codes, or the
report schema. The native curl bridge is intentionally not called here because
its current workspace implementation does not compile and is outside the CLI
and reporting ownership boundary.
