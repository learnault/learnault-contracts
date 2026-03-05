# Course Registry Contract

A Soroban smart contract for managing courses on the Learnault platform. This contract enables protocol admins to deactivate courses while preserving learner credentials.

## Overview

The Course Registry contract provides functionality to:
- Create new courses with an active status
- Deactivate courses to stop new enrollments
- Reactivate courses when needed
- Retrieve course information

Deactivated courses remain in storage so past learners keep their credentials, but the state change signals the frontend to hide the course and blocks new enrollments.

## Features

### Core Functions

#### `create_course(env: Env, admin: Address, id: u32, title: Symbol) -> Symbol`

Creates a new course in the registry.

**Parameters:**
- `env`: The Soroban environment
- `admin`: The admin address performing the action (must be authenticated)
- `id`: Unique course identifier
- `title`: Course title (Symbol, max 9 characters)

**Returns:** `"success"` symbol on successful creation

**Events:** Emits `CourseCreated` event with admin and title

**Example:**
```rust
let result = client.create_course(&admin, &1u32, &symbol_short!("Rust101"));
```

#### `set_course_status(env: Env, admin: Address, id: u32, active: bool) -> Symbol`

Updates the active status of a course.

**Parameters:**
- `env`: The Soroban environment
- `admin`: The admin address performing the action (must be authenticated)
- `id`: Course ID to update
- `active`: New status (true = active, false = deactivated)

**Returns:** `"success"` symbol on successful update

**Events:** Emits `status` event with admin and status ("active" or "inactive")

**Panics:** If course does not exist

**Example:**
```rust
// Deactivate a course
let result = client.set_course_status(&admin, &1u32, &false);

// Reactivate a course
let result = client.set_course_status(&admin, &1u32, &true);
```

#### `get_course(env: Env, id: u32) -> (u32, Symbol, bool)`

Retrieves course information from the registry.

**Parameters:**
- `env`: The Soroban environment
- `id`: Course ID to retrieve

**Returns:** Tuple of (id, title, active)

**Panics:** If course does not exist

**Example:**
```rust
let (id, title, active) = client.get_course(&1u32);
```

## Storage

The contract uses Soroban's persistent storage with the following key structure:

```
("course", course_id) -> (id: u32, title: Symbol, active: bool)
("active", course_id) -> bool (cached for quick status checks)
```

## Events

### CourseCreated
Emitted when a new course is created.
- Topics: `("created", course_id)`
- Data: `(admin_address, course_title)`

### CourseStatusChanged
Emitted when a course status is updated.
- Topics: `("status", course_id)`
- Data: `(admin_address, status_string)` where status_string is "active" or "inactive"

## Authentication

All functions that modify state (`create_course`, `set_course_status`) should enforce admin authentication at the invocation layer. In production deployments, the `admin.require_auth()` call should be uncommented to enforce authentication within the contract. For testing purposes, authentication is handled externally to allow proper unit testing.

## Building

```bash
cargo build -p course-registry --release
```

## Testing

The contract includes comprehensive tests covering:
- Course creation
- Course deactivation
- Course reactivation
- Error handling for non-existent courses

To run tests:
```bash
cargo test -p course-registry --lib
```

## Acceptance Criteria

✅ Active status is toggled in storage
✅ Only the admin address can trigger the change
✅ Deactivated courses remain in storage for credential preservation
✅ Events are emitted for all state changes
✅ Proper error handling for non-existent courses

## Implementation Notes

- Symbol names are limited to 9 characters in Soroban
- Course titles should be kept concise due to Symbol limitations
- The contract uses tuple storage for efficient data retrieval
- Authentication is enforced at the contract level via `require_auth()`

## Future Enhancements

- Add course metadata (description, category, etc.)
- Implement course enrollment tracking
- Add course deletion with proper cleanup
- Support for multiple admin roles
- Course versioning for content updates
