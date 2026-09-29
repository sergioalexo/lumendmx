/** The show's first universe, which today's single-rig UI (top bar quick
 * connect, patch panel) operates on. Additional universes are managed from
 * Setup. Shared in its own module to avoid a circular import between
 * useDmxStore and useShowStore. */
export const PRIMARY_UNIVERSE_ID = 1;
