-- stub for testing; core modules are built-in to scripting engine
local util = require"@taimi/util"

local MenuHandle = {
	i = {},
	mt = {},
}
MenuHandle.mt.__index = MenuHandle.i
function MenuHandle:New(id)
	local MarkerAttributes = require("@taimi/core/attrs").MarkerAttributes
	local i = {
		id = id,
		attrs = MarkerAttributes.new_category(),
	}

	return util.setmetatable(i, self.mt)
end
function MenuHandle.new(...)
	return MenuHandle:New(...)
end
function MenuHandle.i:GetAttrByKey(...)
	return self.attrs:GetAttrByKey(...)
end
function MenuHandle.i:SetAttrByKey(...)
	return self.attrs:SetAttrByKey(...)
end
function MenuHandle.i:UnsetAttrByKey(...)
	return self.attrs:UnsetAttrByKey(...)
end
function MenuHandle.i:GetId()
	local ids = require"@taimi/id"
	return ids.join(self:GetAttrByKey("type"), self:GetAttrByKey("name"))
end
function MenuHandle.i:GetState()
	return rawget(self, "state") == true
end
function MenuHandle.i:SetState(v)
	rawset(self, "state", v)
end
function MenuHandle.i:Unmask()
	error("stub: MenuHandle:Unmask", 2)
end
function MenuHandle.i:Mask()
end

local Menu = {
	i = {},
	mt = {},
}
mt.__index = Menu.i
function Menu:New(i)
	i = i or {}
	if i.children == nil then
		rawset(i, "children", {})
	end

	return util.setmetatable(i, self.mt)
end
function Menu.new(...)
	return Menu:New(...)
end
function Menu.i:GenId(parent, name)
	local ids = require"@taimi/id"
	if name ~= nil then
		name = ids.safe_name(name)
	else
		local count = 1
		for _,_ in pairs(self.children) do
			count = count + 1
		end
		name = string.format("luamenu%03d", count)
	end
	if parent ~= nil then
		name = ids.join(parent, name)
	end
	while self.children[name] do
		name = name .. "_"
	end
	return name
end
function Menu.i:Register(id)
	local handle = MenuHandle.new(id)
	self.children[id] = handle
	return handle
end
function Menu.i:RemoveId(id)
	self.children[id] = nil
end
function Menu.i:LookupId(id)
	return self.children[id]
end

return {
	Menu = Menu,
	MenuHandle = MenuHandle,
}
