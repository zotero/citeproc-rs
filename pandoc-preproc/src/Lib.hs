{-# LANGUAGE OverloadedStrings #-}

module Lib
    ( someFunc
    ) where

import Text.Pandoc.Definition
import Text.Pandoc.Shared (blocksToInlines)
import Text.Pandoc.Class (runPure)
import qualified Text.Pandoc.Readers.Markdown as PM
import qualified Text.Pandoc.JSON as PJ
import qualified Data.Text as T
import qualified Data.Text.IO as TIO
import qualified Data.ByteString as B
import qualified Data.ByteString.Lazy as BL
import Data.Text.Lazy as TL
import Data.Text.Lazy.Encoding (decodeUtf8)

import Data.Default (def)
import Data.Either (fromRight)
import Data.Aeson (toJSON, Value)
import qualified Data.Aeson as A
import Control.Lens
import Data.Aeson.Lens

someFunc :: IO ()
someFunc = readJSON

-- zoom :: IO ()
-- zoom = do
--     result <- runIO $ do
--         doc <- readMarkdown def (T.pack "[testing](https://example.com)")
--         writeJSON

readJSON :: IO ()
readJSON = do
    json <- BL.readFile "lib.json"
    BL.putStr $
        json & values . key "title" %~ parseInlines


    where

        -- unwraps errors and blows up if we can't parse, etc
        parseInlines :: Value -> Value
        parseInlines (A.String x) = fromRight (A.String "") result
            where
                result = runPure $ do
                    doc <- PM.readMarkdown def x
                    let inlines = case doc of
                                    Pandoc meta blocks -> blocksToInlines blocks
                    return $ toJSON inlines
        parseInlines _ = (A.String "")
