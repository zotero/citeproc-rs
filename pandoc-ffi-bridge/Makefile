LIBDIR=$(shell cd ../pandoc-ffi-bridge && stack exec ghc -- --print-libdir)
INC=-I$(LIBDIR)/include
LIB=-L$(LIBDIR)/rts -lHSrts-ghc8.4.4

all: libhs.so

libhs.so:
	@(stack build)

doc:
	@cargo doc
	@(stack haddock)

clean:
	@(stack clean && rm *.so)
