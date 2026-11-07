// Standalone overlay compilation probe. Component registration comes later.
#include <game/client/component.h>

namespace ddmix
{
int ProbeComponentSize()
{
	return sizeof(CComponent);
}
}
