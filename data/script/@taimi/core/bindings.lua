-- stub for testing; core modules are built-in to scripting engine
local util = require"@taimi/util"
local bitop = require"@taimi/bitop"

local GameControls = {
	i = {},
	mt = {},
}
GameControls.mt.__index = GameControls.i
function GameControls:New()
	local i = {
		bits32 = {0,0,0,0,0,0,0,0},
	}
	return util.setmetatable(i, self.mt)
end
function GameControls.new_empty()
	return GameControls:New()
end
function GameControls.i:Bits32At0(idx)
	return self.bits32[idx]
end
function GameControls.i:IsEmpty()
	return not bitop.btest(self.bits32)
end
function GameControls.i:GetAt0(idx)
	error("TODO")
end
function GameControls.i:SetAt0(idx, v)
	error("TODO")
end
function GameControls.i:NextIndexFrom0(idx)
	idx = idx or 0
	error("TODO")
end

local Control = {
	i = {},
	mt = {},
}
function Control.mt:__index(k)
	if k == "Label" then
		return "unknown"
	else
		return self.i[k] or Control.i[k]
	end
end
function Control.mt:__tostring()
	return self.Label
end
-- TODO: __concat with tostring for parity
function Control:New(idx)
	local i = {
		Index = idx,
	}
	return util.setmetatable(i, self.mt)
end
function Control.from_index(idx)
	return Control:New(idx)
end

local ControlNames = {
	Miscellaneous_Interact = Control.from_index(65),
}

return {
	Control = Control,
	ControlNames = ControlNames,
	GameControls = GameControls,
}
