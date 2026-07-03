final: prev: let
  inherit (final) lib stdenv;
  inherit (stdenv.hostPlatform) isMinGW;
  mkLuaname = version: "lua${lib.versions.major version}${lib.versions.minor version}";
  luajitOverrideAttrs = old: let
    luaname = mkLuaname old.luaversion;
    luaShared = false;
    sharedInstall = ''
      install -Dm0755 -t $out/bin/ src/${luaname}.dll || true
      install -Dm0755 -t $out/lib/ src/libluajit-$luaversion.dll.a || true
    '';
    staticFlag = "BUILDMODE=static";
    common = {
      env = old.env // {
        # TODO: bad idea? compare runtime characteristics idk
        NIX_CFLAGS_COMPILE = old.env.NIX_CFLAGS_COMPILE or "" + " -Oz";
      };
    };
  in
    if luaShared
    then common // {
      postInstall = sharedInstall + old.postInstall or "";
    }
    else common // {
      makeFlags = old.makeFlags or [] ++ [staticFlag];
    };
  luaOverrideAttrs = old: let
    luaname = mkLuaname old.version;
    stripDll = true;
    ranlib =
      if stripDll
      then "$STRIP --strip-unneeded"
      else "true";
  in {
    #makeFlags = old.makeFlags or [] ++ [ ];
    preBuild =
      ''
        if grep -qF "RANLIB=strip" src/Makefile; then
          substituteInPlace src/Makefile \
            --replace-warn '"RANLIB=strip --strip-unneeded"' ""
          makeFlagsArray+=(
            "RANLIB=${ranlib}"
          )
        fi
        makeFlagsArray+=(
          MYLIBS=
        )
        installFlagsArray+=(
          TO_BIN="lua.exe luac.exe ${luaname}.dll"
          TO_LIB="lib${luaname}.dll.a liblua.a"
        )
      ''
      + toString old.preBuild or "";
    postBuild =
      ''
        pushd src
        ${stdenv.cc.targetPrefix}dlltool -D ${luaname}.dll -l lib${luaname}.dll.a
        popd
      ''
      + toString old.postBuild or "";
    enableParallelBuilding = true;
    meta =
      old.meta or {}
      // {
        platforms = lib.platforms.all;
      };
  };
in {
  fixW64Lua = lua:
    if isMinGW
    then (lua.overrideAttrs luaOverrideAttrs).override {readline = null;}
    else lua;
  fixW64Luajit = luajit:
    if isMinGW
    then (luajit.overrideAttrs luajitOverrideAttrs).override {
      #enable52Compat = true;
      #deterministicStringIds = true;
      #useSystemMalloc = true; # broken :<
      #enableGC64 = false; # crashy!
    }
    else luajit;
  luajit_2_1 = final.fixW64Luajit prev.luajit_2_1;
  luajit_2_0 = final.fixW64Luajit prev.luajit_2_0;
  lua5_1 = final.fixW64Lua prev.lua5_1;
  lua5_2 = final.fixW64Lua prev.lua5_2;
  lua5_3 = final.fixW64Lua prev.lua5_3;
  lua5_4 = final.fixW64Lua prev.lua5_4;
  lua5_5 = final.fixW64Lua prev.lua5_5;
  # TODO: try lua5_X_compat maybe?
}
