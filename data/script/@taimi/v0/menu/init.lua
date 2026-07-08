local util = require"@taimi/util"

local export = {}

local Menu = {
	i = {},
	mt = {},
	loaders = {}
}
function Menu.mt:__index(k)
	local v = Menu.i[k]
	if v == nil then
		v = Menu.loaders[k]
		if v ~= nil then
			v = v(self)
			if v ~= nil then
				rawset(self, k, v)
			end
		end
	end
	return v
end
function Menu.for_plug(plug)
	local i = {
		plug = plug,
	}
	return util.setmetatable(i, Menu.mt)
end
function Menu.loaders:Plug()
	local menus = require"@taimi/ui/menu"
	return menus.Menu.wrap(self.plug:GetRootMenu())
end
function Menu.loaders:Item()
	return require"@taimi/ui/menu".MenuItem
end

export.Menu = {
	for_plug = Menu.for_plug,
}

return export
