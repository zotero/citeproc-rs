module Lib where
import Curryrs.Types
triple :: I32 -> I32
triple x = 3 * x
foreign export ccall triple :: I32 -> I32

-- import Text.Pandoc.Definition as PD
-- import Data.Int
-- import Foreign.StablePtr
-- import Foreign.Ptr
--
-- newtype Value = Value { unValue :: Int32 }
--
-- foreign export ccall valueCreate :: Int32 -> IO (Ptr ())
-- foreign export ccall valueDestroy :: Ptr () -> IO ()
-- foreign export ccall valueGet :: Ptr () -> IO Int32
--
-- valueCreate :: Int32 -> IO (Ptr ())
-- valueCreate x = castStablePtrToPtr <$> newStablePtr (Value x)
--
-- valueDestroy :: Ptr () -> IO ()
-- valueDestroy p = freeStablePtr sp
--   where sp :: StablePtr Value
--         sp = castPtrToStablePtr p
--
-- valueGet :: Ptr () -> IO Int32
-- valueGet p = unValue <$> deRefStablePtr (castPtrToStablePtr p)


