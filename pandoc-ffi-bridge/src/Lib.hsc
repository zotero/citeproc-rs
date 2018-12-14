{-# LANGUAGE CPP                      #-}
{-# LANGUAGE ForeignFunctionInterface #-}
{-# LANGUAGE EmptyDataDecls           #-}
{-# LANGUAGE TypeFamilies             #-}
{-# LANGUAGE FlexibleInstances        #-}
{-# LANGUAGE FlexibleContexts         #-}
{-# LANGUAGE RecordWildCards          #-}

-- https://wiki.haskell.org/Foreign_Function_Interface#Using_pointers:_Storable_instances

{-

-}

module Lib where

import Text.Pandoc.FFI
import Text.Read (readMaybe)
import Foreign
import Foreign.C.String
import Foreign.C.Types
import Text.Pandoc.Definition as Pd
-- import Data.Text as T
-- import Data.Text.Encoding as E
-- import Data.ByteString (ByteString)
import Curryrs.Types

import Data.Int
import Foreign.StablePtr
import Foreign.Ptr
import qualified Data.ByteString as B
import qualified Data.ByteString.Unsafe as BU
import qualified Data.ByteString.Char8 as Ch

#include <myfile.h>

data HsOwnedString = HsOwnedString { hsoStable :: StablePtr B.ByteString
                                   , hsoString :: CString }

instance Storable HsOwnedString where
    alignment _ = #{alignment struct hs_owned_string}
    sizeOf _ = #{size struct hs_owned_string}
    peek ptr = do
        cs <- (#peek struct hs_owned_string, string) ptr
        sp <- (#peek struct hs_owned_string, stable) ptr
        return HsOwnedString { hsoStable = sp, hsoString = cs }
    poke ptr x = do 
        (#poke struct hs_owned_string, string) ptr (hsoString x)
        (#poke struct hs_owned_string, stable) ptr (hsoStable x)

-- definitely not a good idea to make HSOs out of on non-GC-pin-backed ByteStrings, like one backed by malloc/free.
hsoCreate :: B.ByteString -> IO HsOwnedString
hsoCreate s = do
    sp <- newStablePtr s
    cs <- BU.unsafeUseAsCString s (\x -> return x)
    return HsOwnedString { hsoStable = sp, hsoString = cs }

foreign export ccall get_hso :: Ptr HsOwnedString -> IO ()
get_hso :: Ptr HsOwnedString -> IO ()
get_hso ptr = do
    line <- B.getLine
    hso <- hsoCreate line
    poke ptr hso
    return ()

foreign export ccall stable_ptr_drop :: StablePtr () -> IO ()
stable_ptr_drop :: StablePtr () -> IO ()
stable_ptr_drop ptr = do
    putStrLn "dropping hso"
    freeStablePtr ptr

foreign export ccall tiny_parse :: CString -> Ptr HsOwnedString -> IO ()
tiny_parse :: CString -> Ptr HsOwnedString -> IO ()
tiny_parse x ptr = do
    a <- peekCString x
    let z = case (readMaybe a :: Maybe Integer) of
                Just 0 -> "zero"
                Just 1 -> "one"
                Just 2 -> "two"
                Just n -> "I think that was a number, but I can't count that high"
                otherwise -> "I was not expecting that"
    hso <- hsoCreate $ Ch.pack z
    poke ptr hso

{-
   Haskell CString is not guaranteed to stay alive in the view of the garbage
   collector, because it is only a typedef for Ptr CChar. So when we construct
   strings and send them back to Rust, we want to be sure they aren't reclaimed
   in a GC cycle by the RTS before they are used.

   So, roughly following this approach:

   https://www.reddit.com/r/haskell/comments/6rxc4i/is_there_any_reliable_way_to_pass_bytestring/dl8iity/
   https://github.com/lyokha/nginx-haskell-module/blob/62b9fda2d80b8f417678616e2fbadd748b5d9ece/haskell/ngx-export/NgxExport.hs#L538

   ... we shall have:

   1. a strict ByteString holding a reference to some pinned data
   2. a StablePtr to the ByteString, so that the ByteString does not die in the
      GC's view, and neither does its backing array
   3. a CString pointing to the pinned data

   Package 2 and 3 in a struct, and export a function for Rust to free the
   stableptr.

   The rust version will appear to be an owned type, and will include a foreign
   call in the drop() implementation to tell Haskell it can safely free the
   ByteString again.
-}

-- https://github.com/mgattozzi/curryrs/issues/5



