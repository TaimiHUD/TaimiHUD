-- stub for testing; core modules are built-in to scripting engine
local Mumble = {
	IsAvailable = true,
	CurrentMap = {
		-- Id = 1510,
		Id = 1437, -- harvest temple (hmpSim testing)
		-- Id = 50,
		Type = 0,
		IsCompetitiveMode = false,
	},
	Info = {
		BuildId = 0,
		IsGameFocused = true,
	},
	PlayerCamera = {
		FieldOfView = 1.0,
		NearPlaneRenderDistance = 0.6,
		FarPlaneRenderDistance = 800.0,
	},
	PlayerCharacter = {
		Name = "mew",
		Race = 0,
		Specialization = 0,
		TeamColorId = 0,
		CurrentMount = 0,
		IsCommander = false,
		IsInCombat = false,
		Profession = 0,
	},
	UI = {
		CompassRotation = 0.0,
		CompassSize = {
			Width = 256,
			Height = 256,
		},
		IsCompassRotationEnabled = false,
		IsCompassTopRight = false,
		IsMapOpen = false,
		IsTextInputFocused = false,
		MapCenter = {
			X = 0.0,
			Y = 0.0,
		},
		MapPosition = {
			X = 0.0,
			Y = 0.0,
		},
		MapScale = 1.0,
		UISize = 0,
	},
}

local vectors = require"@taimi/core/vectors"
Mumble.PlayerCharacter.Forward = vectors.Vec3(0.0, 0.0, 1.0)
Mumble.PlayerCharacter.Position = vectors.Vec3(0.0, 0.0, 0.0)
Mumble.PlayerCamera.Forward = vectors.Vec3(0.0, 0.0, 1.0)
Mumble.PlayerCamera.Position = vectors.Vec3(0.0, 1.0, -1.0)

return {
	Mumble = Mumble,
}
