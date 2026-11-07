#ifndef GAME_CLIENT_COMPONENTS_DDMIX_EXAMPLE_H
#define GAME_CLIENT_COMPONENTS_DDMIX_EXAMPLE_H
#include <game/client/component.h>

class CDdmixExample : public CComponent
{
public:
	int Sizeof() const override { return sizeof(*this); }
	void OnInit() override;
};
#endif
