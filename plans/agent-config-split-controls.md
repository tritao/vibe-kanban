# Split Agent Configuration UI (Model / Effort / Approvals)

## Problem

The UI currently exposes agent configuration as a single “Configuration / Variant” dropdown (e.g.
`DEFAULT`, `HIGH`, `APPROVALS`, `MAX`). This is hard to reason about because each “variant” is really
a bundle of underlying settings (model, reasoning effort, approval policy, sandbox, etc.).

Operators want to choose these dimensions explicitly:

- **Model** (e.g. `gpt-5.2`, `gpt-5.2-codex`, `gpt-5.1-codex-max`)
- **Effort** (e.g. `high`)
- **Approvals** (e.g. `unless-trusted`, `on-request`)

…without needing to remember which preset name encodes which behavior.

## Goals

- Replace the single “variant” selector with **three explicit controls**: Model / Effort / Approvals.
- Keep the backend contract unchanged: task creation and agent runs still reference
  `ExecutorProfileId { executor, variant }`.
- Preserve existing “preset” variants as first-class, but make them *derived* from the three knobs.
- Make this work across all places where we choose an executor profile:
  - Onboarding “Choose Your Coding Agent”
  - Settings → “Default Coding Agent”
  - Task creation dialog (auto-start attempts)

## Non-goals (v1)

- Arbitrary “compose any combination” without first having a matching variant defined in profiles.
  (We can add “Create preset from current selection” in v2.)
- Rewriting shared types or adding new API payload shapes.

## Current reality (important constraints)

- The runtime selection is `ExecutorProfileId` (executor + optional variant).
- The actual configuration for each variant lives in **executor profiles** (merged defaults +
  overrides) returned by `/api/config/profiles` and `/api/config`.
- For Codex, the fields we care about already exist in the profile JSON:
  - `CODEX.model`
  - `CODEX.model_reasoning_effort`
  - `CODEX.ask_for_approval`
  - (related, but not requested in UI: `CODEX.sandbox`)

## UX proposal

### High-level layout

Keep the **Agent** selector as-is, but replace **Configuration** with:

1. **Model** (dropdown)
2. **Effort** (dropdown or segmented)
3. **Approvals** (dropdown)

Plus a small, optional “Preset” indicator for transparency:

- `Preset: HIGH` (read-only chip / tooltip), because the system still ultimately chooses a named
  variant under the hood.

### Interaction model (v1: variant-backed)

The three controls do not directly write arbitrary values; they are a *lens* over existing variants:

- When the user changes one control, the UI selects the **best matching existing variant** and sets
  `ExecutorProfileId.variant` accordingly.
- The option lists are filtered so the user can only select combinations that exist in profiles.
  (This avoids a “custom unsaved” state that cannot be represented in `ExecutorProfileId`.)

### Advanced / creation (v2)

Add a `⋯` menu:

- `Create preset from current selection…` (writes a new variant into profiles via `/api/config/profiles`)
- `Manage agent configurations…` (deep link to Settings → Agents)

## Scope: which executors get split controls?

### v1: Codex only

Codex is the main driver of this request and has clear fields for model/effort/approvals.

For other executors:

- Keep the existing “Configuration” dropdown for now.
- Later, we can generalize “Approvals” (e.g. Gemini `yolo`, Claude `approvals`, Opencode `auto_approve`)
  but these are not equivalent semantics and should be treated carefully.

## Frontend implementation plan

### 1) Introduce a shared selector component

Create `frontend/src/components/tasks/CodexVariantControls.tsx` (name TBD) that replaces
`ConfigSelector` *when* `selectedExecutorProfile.executor === 'CODEX'`.

Inputs:

- `profiles`: `Record<string, ExecutorConfig>` (from `useUserSystem()`)
- `selectedExecutorProfile`: `ExecutorProfileId | null`
- `onChange(profile: ExecutorProfileId)`
- `disabled`, `showLabel`, etc.

Outputs:

- Calls `onChange` with the same executor but the chosen `variant` (or `null` for `DEFAULT`).

### 2) Extract Codex variant metadata

Add helper(s) in `frontend/src/lib/executorProfiles.ts` (or colocated):

- `getCodexVariants(executorConfig: ExecutorConfig): Array<{ variantName, model, effort, approvals, sandbox }>`
  - Only include entries where the value is `{ CODEX: Codex }`.
  - Normalize `DEFAULT` semantics:
    - If `variantName === 'DEFAULT'`, treat it as selectable but store `ExecutorProfileId.variant = null`.
  - Treat `null`/`undefined` as “Default” for each field.

### 3) Build the option sets (dependent filtering)

Given the list of variants:

- `modelOptions`: distinct models + `Default`.
- `effortOptionsForModel(model)`: distinct efforts among variants that match model (+ `Default`).
- `approvalOptionsForModelAndEffort(model, effort)`: distinct approvals among matching variants (+ `Default`).

This ensures the UI never produces an invalid triple.

### 4) Selection algorithm (“best matching variant”)

When one knob changes:

1. Compute the desired `(model, effort, approvals)` triple.
2. Find variants matching all selected dimensions.
3. If multiple, prefer in order:
   - exact match including `sandbox` (if present)
   - `DEFAULT`
   - shortest variant name / stable sort (deterministic)
4. Set `ExecutorProfileId.variant` to:
   - `null` if chosen variant is `DEFAULT`
   - else the chosen variant string

### 5) Replace existing uses of ConfigSelector where appropriate

Update `frontend/src/components/settings/ExecutorProfileSelector.tsx:1`:

- Keep `AgentSelector`.
- Replace `ConfigSelector` with:
  - `CodexVariantControls` for Codex
  - `ConfigSelector` fallback for non-Codex

This automatically updates:

- Task creation dialog (it uses `ExecutorProfileSelector`)
- Settings surfaces that reuse it

### 6) Update onboarding dialog

`frontend/src/components/dialogs/global/OnboardingDialog.tsx:1` currently implements its own agent +
variant UI. Replace that selection block with `ExecutorProfileSelector` to keep behavior consistent.

### 7) Labels + tooltips (operators-only clarity)

Add concise hover tooltips:

- Model: “Which model to run”
- Effort: “More effort = slower, more thorough”
- Approvals: “When to ask before risky actions”

If `Approvals` implies `sandbox` changes (e.g. CODEX `APPROVALS` sets `workspace-write`), show a
secondary line in the tooltip: “This preset also sets sandbox: workspace-write” (derived from the
chosen variant metadata).

## Backend / API plan

### v1: no backend changes required

All values continue to be represented by selecting an existing variant; requests continue sending:

- `ExecutorProfileId.executor`
- `ExecutorProfileId.variant` (or `null` for `DEFAULT`)

### v2: create presets from UI (optional)

If we want “compose any combo”:

- Add a flow to create a new variant in profiles:
  - Load `ExecutorConfigs` from `/api/config/profiles`
  - Create/overwrite a new variant name under `executors.CODEX.<NAME>.CODEX` with chosen fields
  - PUT the updated `ExecutorConfigs` back to `/api/config/profiles`
  - Reload system (`useUserSystem().reloadSystem()`)

Reuse `CreateConfigurationDialog` for naming/validation.

## Edge cases / risks

- Profiles may omit a dimension:
  - If no variants specify `model_reasoning_effort`, Effort control should collapse to a disabled
    “Default” state.
- Profiles may include variants that set unrelated fields:
  - The split UI should ignore unrelated fields, but preserve determinism by selecting a single
    variant consistently.
- “Default” semantics differ per executor:
  - For Codex, leaving `model` unset may still result in an internal default; show “Default” but keep
    preset mapping stable.

## Testing plan

Frontend:

- Unit test the variant selection algorithm given a synthetic `ExecutorConfig`.
- Integration test (if present) for `ExecutorProfileSelector`:
  - Switching model/effort/approvals yields the expected `ExecutorProfileId.variant`.
  - No invalid states: every UI selection corresponds to an existing variant.

Manual:

- Onboarding: pick Codex + approvals; verify config saved and used for first attempt.
- Task create: verify `ExecutorProfileId` changes as expected when toggling controls.

## Rollout plan

1. Implement Codex split controls in `ExecutorProfileSelector`.
2. Migrate onboarding to use the same selector.
3. Add the “Preset” indicator for transparency.
4. Optional: add “Create preset…” + link to agent settings.

