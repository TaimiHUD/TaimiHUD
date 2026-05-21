-- stub for testing; core modules are built-in to scripting engine
return {
	Print = print,
	Info = function(...) print("INFO", ...) end,
	Warn = function(...) print("WARN", ...) end,
	Error = function(...) print("ERR!", ...) end,
}
