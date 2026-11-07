	// This is the body of a separate static void helper, not SnapInput itself.
	// Bindings supply a pointer, an argument and a reference to selected state.
	// Assignment through player_flags would update the original member.
	(void)controls;
	(void)data;
	(void)player_flags;
	return; // Returns from this helper only; SnapInput continues normally.
