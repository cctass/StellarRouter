# API Reference

Complete reference for all public functions across the six stellar-router contracts.

---

## router-core

**Contract:** `RouterCore`  
**Purpose:** Central dispatcher — registers routes by name and resolves them to contract addresses.

### `initialize(admin: Address) → Result<(), RouterError>`
Sets up the admin, marks the router as unpaused, and resets the total-routed counter.  
Must be called exactly once before any other function.

**Errors:** `AlreadyInitialized`

```bash
stellar contract invoke --id <CORE_ID> --network testnet --source admin \
  -- initialize --admin <ADMIN_ADDRESS>
```

---

### `register_route(caller, name, address) → Result<(), RouterError>`
Registers a new route. `name` must be unique and non-empty. Caller must be admin.

**Errors:** `Unauthorized`, `RouteAlreadyExists`, `NotInitialized`, `InvalidRouteName` (empty/whitespace)

```bash
stellar contract invoke --id <CORE_ID> --network testnet --source admin \
  -- register_route --caller <ADMIN> --name oracle --address <CONTRACT_ID>
```

---

### `update_route(caller, name, new_address) → Result<(), RouterError>`
Updates an existing route to point to a new address. Emits `route_updated` and `route_overwritten` events.

**Errors:** `Unauthorized`, `RouteNotFound`, `NotInitialized`

---

### `remove_route(caller, name) → Result<(), RouterError>`
Deletes a route and removes any aliases pointing to it.

**Errors:** `Unauthorized`, `RouteNotFound`, `NotInitialized`

---

### `resolve(name) → Result<Address, RouterError>`
Resolves a route name (or alias) to its contract address. Increments `total_routed`.

**Errors:** `RouterPaused`, `RouteNotFound`, `RoutePaused`

```bash
stellar contract invoke --id <CORE_ID> --network testnet --source any \
  -- resolve --name oracle
```

---

### `set_route_paused(caller, name, paused: bool) → Result<(), RouterError>`
Pauses or unpauses a specific route.

**Errors:** `Unauthorized`, `RouteNotFound`, `NotInitialized`

---

### `set_paused(caller, paused: bool) → Result<(), RouterError>`
Pauses or unpauses the entire router. Overrides individual route state.

**Errors:** `Unauthorized`, `NotInitialized`

---

### `get_route(name) → Option<RouteEntry>`
Returns the full `RouteEntry` for `name`, or `None` if not registered.

---

### `get_all_routes() → Vec<String>`
Returns all registered route names.

---

### `add_alias(caller, existing_name, alias_name) → Result<(), RouterError>`
Creates an alias for an existing route. Resolving the alias returns the original route's address.

**Errors:** `Unauthorized`, `RouteNotFound` (existing_name), `RouteAlreadyExists` (alias_name)

---

### `remove_alias(caller, alias_name) → Result<(), RouterError>`
Removes an alias.

**Errors:** `Unauthorized`, `RouteNotFound`

---

### `total_routed() → u64`
Returns the cumulative count of successful `resolve` calls.

---

### `admin() → Result<Address, RouterError>`
Returns the current admin address.

**Errors:** `NotInitialized`

---

### `transfer_admin(current, new_admin) → Result<(), RouterError>`
Transfers admin to a new address. Emits `admin_transferred`.

**Errors:** `Unauthorized`, `NotInitialized`

---

## router-registry

**Contract:** `RouterRegistry`  
**Purpose:** Versioned address book — stores contract addresses keyed by `(name, version)`.

### `initialize(admin) → Result<(), RegistryError>`
**Errors:** `AlreadyInitialized`

---

### `register(caller, name, address, version: u32) → Result<(), RegistryError>`
Registers a contract entry. `version` must be > 0 and greater than all existing versions for `name`.

**Errors:** `Unauthorized`, `InvalidVersion`, `AlreadyRegistered`, `NotInitialized`

```bash
stellar contract invoke --id <REGISTRY_ID> --network testnet --source admin \
  -- register --caller <ADMIN> --name oracle --address <CONTRACT_ID> --version 1
```

---

### `get(name, version: u32) → Result<ContractEntry, RegistryError>`
Returns the entry for `(name, version)`.

**Errors:** `NotFound`

---

### `get_latest(name) → Result<ContractEntry, RegistryError>`
Returns the highest non-deprecated version for `name`.

**Errors:** `NotFound`

---

### `get_latest_with_constraint(name, constraint: Option<String>) → Result<ContractEntry, RegistryError>`
Returns the highest non-deprecated version matching a semver constraint (`>=X`, `<=X`, `>X`, `<X`, `^X`, `~X`, or exact).

**Errors:** `NotFound`, `InvalidConstraint`

---

### `deprecate(caller, name, version: u32) → Result<(), RegistryError>`
Marks a version as deprecated. Deprecated versions are skipped by `get_latest`.

**Errors:** `Unauthorized`, `VersionNotFound`, `AlreadyDeprecated`, `NotInitialized`

---

### `deprecate_many(caller, entries: Vec<(String, u32)>) → Vec<Result<(), RegistryError>>`
Batch deprecation. Returns one result per entry; partial failures are allowed.

---

### `versions(name) → Vec<u32>`
Returns all registered version numbers for `name` in ascending order.

---

### `admin() → Result<Address, RegistryError>`
**Errors:** `NotInitialized`

---

### `transfer_admin(current, new_admin) → Result<(), RegistryError>`
**Errors:** `Unauthorized`, `NotInitialized`

---

## router-access

**Contract:** `RouterAccess`  
**Purpose:** Role-based access control with optional role expiry and blacklisting.

### `initialize(super_admin) → Result<(), AccessError>`
**Errors:** `AlreadyInitialized`

---

### `grant_role(caller, role, target, expires_at: Option<u64>) → Result<(), AccessError>`
Grants `role` to `target`. `expires_at` is an optional ledger sequence number after which the role expires. Caller must be super-admin or role admin.

**Errors:** `Unauthorized`, `AlreadyHasRole`, `Blacklisted`

---

### `revoke_role(caller, role, target) → Result<(), AccessError>`
Revokes `role` from `target`. Removes the storage key (not just sets to false).

**Errors:** `Unauthorized`, `RoleNotFound`

---

### `has_role(role, target) → bool`
Returns `true` if `target` holds `role` and it has not expired. Returns `false` for blacklisted addresses.

---

### `set_role_admin(caller, role, admin) → Result<(), AccessError>`
Designates `admin` as the address that can grant/revoke `role`. Only super-admin can call this.

**Errors:** `Unauthorized`, `NotInitialized`

---

### `blacklist(caller, target) → Result<(), AccessError>`
Prevents `target` from being granted any role. Cannot blacklist the super-admin.

**Errors:** `Unauthorized`, `CannotBlacklistAdmin`, `NotInitialized`

---

### `unblacklist(caller, target) → Result<(), AccessError>`
Removes `target` from the blacklist.

**Errors:** `Unauthorized`, `NotInitialized`

---

### `is_blacklisted(target) → bool`

---

### `get_role_members(role) → Vec<Address>`
Returns all addresses currently holding `role`.

---

### `get_roles_for_address(addr) → Vec<String>`
Returns all roles held by `addr`.

---

### `expire_role(caller, role, target) → Result<(), AccessError>`
Removes the expiry entry for a role grant (cleanup function). Only super-admin.

**Errors:** `Unauthorized`, `NotInitialized`

---

### `super_admin() → Result<Address, AccessError>`
**Errors:** `NotInitialized`

---

### `transfer_super_admin(current, new_admin) → Result<(), AccessError>`
**Errors:** `Unauthorized`, `NotInitialized`

---

## router-middleware

**Contract:** `RouterMiddleware`  
**Purpose:** Pre/post call hooks with rate limiting and circuit breaker.

### `initialize(admin) → Result<(), MiddlewareError>`
**Errors:** `AlreadyInitialized`

---

### `configure_route(caller, route, max_calls_per_window, window_seconds, enabled, failure_threshold, recovery_window_seconds) → Result<(), MiddlewareError>`
Configures rate limiting and circuit breaker for a route. Set `max_calls_per_window = 0` to disable rate limiting. Set `failure_threshold = 0` to disable the circuit breaker.

**Errors:** `Unauthorized`, `InvalidConfig` (window_seconds=0 with max_calls>0), `NotInitialized`

```bash
stellar contract invoke --id <MIDDLEWARE_ID> --network testnet --source admin \
  -- configure_route \
  --caller <ADMIN> --route oracle/get_price \
  --max_calls_per_window 100 --window_seconds 3600 \
  --enabled true --failure_threshold 5 --recovery_window_seconds 300
```

---

### `pre_call(caller, route) → Result<(), MiddlewareError>`
Must be called before routing. Validates global enable, route enable, circuit breaker, and rate limit. Increments `total_calls` on success.

**Errors:** `MiddlewareDisabled`, `RouteDisabled`, `CircuitOpen`, `RateLimitExceeded`

---

### `post_call(caller, route, success: bool)`
Must be called after routing. Emits `post_call` event. Increments circuit breaker failure count on failure.

---

### `set_global_enabled(caller, enabled: bool) → Result<(), MiddlewareError>`
Globally enables or disables all middleware.

**Errors:** `Unauthorized`, `NotInitialized`

---

### `reset_circuit_breaker(caller, route) → Result<(), MiddlewareError>`
Manually resets the circuit breaker for a route.

**Errors:** `Unauthorized`, `NotInitialized`

---

### `total_calls() → u64`
Cumulative count of successful `pre_call` invocations.

---

### `rate_limit_state(route, caller) → Option<RateLimitState>`
Returns the current rate limit state for `(route, caller)`.

---

### `route_config(route) → Option<RouteConfig>`
Returns the middleware config for `route`.

---

### `admin() → Result<Address, MiddlewareError>`
**Errors:** `NotInitialized`

---

### `transfer_admin(current, new_admin) → Result<(), MiddlewareError>`
**Errors:** `Unauthorized`, `NotInitialized`

---

## router-timelock

**Contract:** `RouterTimelock`  
**Purpose:** Delayed execution queue — all sensitive changes must wait a configurable delay.

### `initialize(admin, min_delay: u64) → Result<(), TimelockError>`
`min_delay` must be > 0 (seconds).

**Errors:** `AlreadyInitialized`, `InvalidDelay`

---

### `queue(proposer, description, target, delay: u64, depends_on: Vec<u64>) → Result<u64, TimelockError>`
Queues a new operation. Returns the operation ID. `delay` must be >= `min_delay`. `depends_on` lists operation IDs that must execute first.

**Errors:** `Unauthorized`, `InvalidDelay`, `NotInitialized`

```bash
stellar contract invoke --id <TIMELOCK_ID> --network testnet --source admin \
  -- queue \
  --proposer <ADMIN> --description "upgrade oracle to v2" \
  --target <CONTRACT_ID> --delay 86400 --depends_on "[]"
```

---

### `execute(caller, op_id: u64) → Result<(), TimelockError>`
Executes a queued operation after its ETA. All dependencies must be executed first.

**Errors:** `Unauthorized`, `NotFound`, `AlreadyExecuted`, `AlreadyCancelled`, `TooEarly`, `DependencyNotMet`

---

### `cancel(caller, op_id: u64) → Result<(), TimelockError>`
Cancels a queued operation. Clears its dependency list.

**Errors:** `Unauthorized`, `NotFound`, `AlreadyExecuted`, `AlreadyCancelled`

---

### `cancel_all(admin) → Result<u32, TimelockError>`
Cancels all pending (not executed, not cancelled) operations. Returns the count cancelled.

**Errors:** `Unauthorized`, `NotInitialized`

---

### `get_op(op_id: u64) → Option<TimelockOp>`
Returns the operation, or `None` if not found.

---

### `min_delay() → Result<u64, TimelockError>`
**Errors:** `NotInitialized`

---

### `set_min_delay(caller, new_delay: u64) → Result<(), TimelockError>`
Updates the minimum delay. Does not affect already-queued operations.

**Errors:** `Unauthorized`, `InvalidDelay`, `NotInitialized`

---

### `admin() → Result<Address, TimelockError>`
**Errors:** `NotInitialized`

---

### `transfer_admin(current, new_admin) → Result<(), TimelockError>`
**Errors:** `Unauthorized`, `NotInitialized`

---

## router-multicall

**Contract:** `RouterMulticall`  
**Purpose:** Batch multiple cross-contract calls in a single transaction.

### `initialize(admin, max_batch_size: u32) → Result<(), MulticallError>`
`max_batch_size` must be > 0.

**Errors:** `AlreadyInitialized`, `InvalidConfig`

---

### `execute_batch(caller, calls: Vec<CallDescriptor>, simulate: bool) → Result<BatchSummary, MulticallError>`
Executes a batch of calls. Any authenticated address can call this (not admin-only). If `simulate = true`, calls are attempted but `total_batches` is not incremented. If a call with `required = true` fails, the batch aborts immediately.

**Errors:** `EmptyBatch`, `BatchTooLarge`, `RequiredCallFailed`, `NotInitialized`

---

### `set_max_batch_size(caller, max_batch_size: u32) → Result<(), MulticallError>`
**Errors:** `Unauthorized`, `InvalidConfig`, `NotInitialized`

---

### `total_batches() → u64`
Cumulative count of non-simulated `execute_batch` calls.

---

### `max_batch_size() → Result<u32, MulticallError>`
**Errors:** `NotInitialized`

---

### `admin() → Result<Address, MulticallError>`
**Errors:** `NotInitialized`

---

### `transfer_admin(current, new_admin) → Result<(), MulticallError>`
**Errors:** `Unauthorized`, `NotInitialized`

---

## Common Types

### `RouteEntry`
| Field | Type | Description |
|---|---|---|
| `address` | `Address` | Resolved contract address |
| `name` | `String` | Route name |
| `paused` | `bool` | Whether this route is paused |
| `updated_by` | `Address` | Last admin to update this route |

### `ContractEntry`
| Field | Type | Description |
|---|---|---|
| `address` | `Address` | Registered contract address |
| `name` | `String` | Human-readable name |
| `version` | `u32` | Version number |
| `deprecated` | `bool` | Whether deprecated |
| `registered_by` | `Address` | Who registered it |

### `CallDescriptor`
| Field | Type | Description |
|---|---|---|
| `target` | `Address` | Contract to call |
| `function` | `Symbol` | Function name |
| `required` | `bool` | Abort batch on failure |
| `instruction_budget` | `Option<u64>` | Reserved for future budget metering |

### `BatchSummary`
| Field | Type | Description |
|---|---|---|
| `total` | `u32` | Total calls attempted |
| `succeeded` | `u32` | Successful calls |
| `failed` | `u32` | Failed calls |
| `budget_exceeded_count` | `u32` | Failed calls that had a budget set |
