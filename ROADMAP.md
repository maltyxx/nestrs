# Roadmap

NestRS is in **alpha** — the foundations are in place and the API still shifts.
This is a *direction, not a dated commitment*; priorities move with what the
community needs.

Want to shape it? Open a
[Discussion](https://github.com/maltyxx/nestrs/discussions) or pick up a
[`good first issue`](https://github.com/maltyxx/nestrs/labels/good%20first%20issue).
The authoritative record of *what was decided and why* is
[CLAUDE.md](CLAUDE.md); this file tracks *what's next*.

## Recently shipped

- **`nestrs-testing`** — an in-process testing module that boots the real DI
  graph and fires HTTP / GraphQL requests inside `cargo test`, with provider
  overrides for mocking (the `Test.createTestingModule` analog).
- **Richer boot diagnostics** — the DI graph names the offending provider and the
  missing dependency, distinguishes a missing provider from a dependency cycle,
  and rejects a non-`Arc` `#[inject]` at compile time.
- **Per-handler metadata + `Reflector`** — `#[meta(...)]` on a handler, read back
  by a guard via `nestrs_http::Reflector` (the `@Roles` / `@SetMetadata` analog).
- **GraphQL authorization** — `nestrs-authz-graphql` gates resolvers with the
  request-scoped `Ability`, carried into the GraphQL context by a per-request
  bridge in `nestrs-graphql`.
- **CORS** — `HttpTransport::cors(...)` (the `app.enableCors` analog).

## Now — stabilising the alpha

- Settle the public API of the core crates so early adopters stop chasing
  breaking changes.
- **Published benchmarks** — replace the "native-Rust-vs-Node" framing with
  reproducible throughput, memory, and cold-start numbers.
- Fill in crate-level docs and grow the `apps/` examples.

## Next — the documented gaps

These are known, deliberate omissions called out in the code today:

- **OpenAPI** — query-parameter schemas, real path-parameter *types* (emitted as
  `string` for now), security schemes, and a committed `openapi.json` snapshot
  written on boot (mirroring how the GraphQL SDL is committed).
- **Scheduling** — cron expressions. Today `#[cron_job]` takes fixed intervals
  only (`ms` / `s` / `m` / `h`), pending a parser that clears the dependency bar.
- **Dependency-injection scopes** — request- and transient-scoped providers. The
  container is singleton-only today; per-request state is carried ad hoc through
  extractors and request-scoped DataLoaders.
- **Events** — an event bus and an `#[event_handler]` decorator, the
  discovered-concern analog of `@nestjs/event-emitter`.
- **`nestrs-resource`** — relations, enums, and pagination types for the
  entity-to-API resource macro, which is experimental today.
- **API versioning** — per-route / per-controller version selection. (CORS and
  global exception filters already ship — `HttpTransport::cors` and the
  middleware `Filter` category, the `@Catch` analog.)
- **Config** — a validated, injectable config service, plus optional
  dependencies (`Option<Arc<T>>`, the `@Optional` analog).

## Later — exploring

- GraphQL **federation**, and the dedicated schema tooling it would reintroduce.
- More transports and surfaces as the discovery model proves out.

## Not on the roadmap

By design — see the *Hard "no" list* in [CLAUDE.md](CLAUDE.md):

- No external dependency-injection library — the container is ours.
- No splitting the workspace into microservices "for scalability".
- No backwards-compatibility shims while the API is pre-1.0.
