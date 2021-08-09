#
# Build the dylib, modules and headers
#
#
SUFFIXES += .d

RUST_FFI_SOURCES = $(wildcard src/*.rs) $(wildcard src/*/*.rs)

MODULES_H = c.h swift.h
MODULES_HPP = cpp.hpp

MODULE_MAPS_H = $(patsubst %.h,modules/%/include/module.modulemap,$(MODULES_H))
MODULE_MAPS_HPP = $(patsubst %.hpp,modules/%/include/module.modulemap,$(MODULES_HPP))
MODULE_HEADERS_H = $(patsubst %.h,modules/%/include/citeproc_rs.h,$(MODULES_H))
MODULE_HEADERS_HPP = $(patsubst %.hpp,modules/%/include/citeproc_rs.hpp,$(MODULES_HPP))

RPATH=$(PWD)/../../target/debug
DYLIB = $(RPATH)/libciteproc_rs.dylib

.PHONY: all
all: $(DYLIB) modules examples

.PHONY: modules
modules: $(MODULE_HEADERS_H) $(MODULE_HEADERS_HPP) $(MODULE_MAPS_H) $(MODULE_MAPS_HPP)

modules/%/include/citeproc_rs.h: cbindgen.%.toml $(RUST_FFI_SOURCES)
	cbindgen -c $< -o $@
	@touch $@
modules/%/include/citeproc_rs.hpp: cbindgen.%.toml $(RUST_FFI_SOURCES)
	cbindgen -c $< -o $@
	@touch $@

$(MODULE_MAPS_H):
	echo 'module CiteprocRs {\n    header "citeproc_rs.h"\n    export *\n}' > $@
$(MODULE_MAPS_HPP):
	echo 'module CiteprocRs {\n    header "citeproc_rs.hpp"\n    export *\n}' > $@

#
# Build the example clients
#

LDFLAGS = -L$(RPATH) -Wl,-rpath,$(RPATH)
LIBS = -lciteproc_rs
CFLAGS = -I./modules/c/include
CXXFLAGS = -std=c++11 -stdlib=libc++ -I./modules/cpp/include
# this also works, but it's a bit bleeding edge
# CXXFLAGS = -std=c++2a -stdlib=libc++ -fmodules -fbuiltin-module-map -fimplicit-modules \
	-fmodule-map-file=modules/cpp/include/module.modulemap

# technically this will not capture all times cargo wants to rebuild
.PHONY: $(DYLIB)
$(DYLIB): $(RUST_FFI_SOURCES)
	cargo build -p citeproc-ffi --features testability

examples = $(wildcard examples/*.c) $(wildcard examples/*.cpp)
example_bins = $(patsubst examples/%.cpp,bin/%,$(patsubst examples/%.c,bin/%,$(examples)))
example_objs = $(patsubst examples/%,build/examples/%.o,$(example_bins))

# we will write these depfiles with -MMD etc below
example_deps = $(patsubst examples/%,build/examples/%.d,$(wildcard examples/*))
-include $(example_deps)

build/examples/%.cpp.o: examples/%.cpp
	@mkdir -p build/examples
	$(CXX) -c $(CXXFLAGS) $(CPPFLAGS) -o $@ -MMD -MFbuild/examples/$*.cpp.d $<
build/examples/%.c.o: examples/%.c
	mkdir -p build/examples
	$(CC) -c $(CFLAGS) $(CPPFLAGS) -o $@ -MMD -MFbuild/examples/$*.c.d $<

bin/client: build/examples/client.c.o
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $< $(LIBS)
bin/client_cpp: build/examples/client_cpp.cpp.o
	$(CXX) $(CXXFLAGS) $(LDFLAGS) -o $@ $< $(LIBS)
bin/panicking: build/examples/panicking.c.o
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $< $(LIBS)

.PHONY: examples
examples: bin $(example_bins)
bin:
	mkdir -p bin

.PHONY: clean
clean:
	rm -rf build $(example_bins)
	rm -rf modules
	touch src/lib.rs

.PHONY: git
git: clean modules

