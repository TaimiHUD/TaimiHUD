-- stub for testing; core modules are built-in to scripting engine
local SemVer = {
	i = {},
}
SemVer.mt = { __index = SemVer.i }
function SemVer:New(version, i)
	local util = require"@taimi/util"
	i = util.setmetatable(i or {}, self.mt)
	i.version = version
	return i
end
function SemVer.new(...)
	return SemVer:New(...)
end
function SemVer.i:IsVersionAtLeast(v)
	return true
end
function SemVer.i:Scrub()
	return SemVer:New(string.match(self.version, "^%d+%.%d+%.%d+"))
end
function SemVer.mt:__tostring()
	return self.version
end

return {
	SemVer = SemVer,
	Taimi = SemVer:New("0.5.0"),
	Api = SemVer:New("0.0.1"),
	Compat = SemVer:New("1.11.999+taimi"),
}
