# Semantic IR

Semantic compilation derives roles, names, geometry, visibility, actions, and
provenance from observed browser state. `SemanticIr::normalize` canonicalizes
labels and reports two classes of unresolved information: duplicate stable
identities and missing accessible names on interactive controls. Callers must
resolve those issues before selecting a target; normalization never chooses an
arbitrary duplicate.
