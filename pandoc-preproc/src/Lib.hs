{-# LANGUAGE OverloadedStrings #-}

module Lib
    ( parseTextFields
    ) where

import Text.Pandoc.Definition
import Text.Pandoc.Shared (blocksToInlines)
import Text.Pandoc.Class (runPure)
import qualified Data.Set as Set
import qualified Data.Vector as V
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

textFields :: Set.Set T.Text
textFields = Set.fromList [
      -- these should probably be parsed as Blocks
      "annote",
      "note",
      "abstract",

      -- these must be verbatim, so don't parse them
      -- "URL",
      -- "PMCID",
      -- "PMID",
      -- "ISBN",
      -- "ISSN",
      -- "DOI",

      "archive",
      "archive-location",
      "archive-place",
      "authority",
      "call-number",
      "citation-label",
      "collection-title",
      "container-title",
      "container-title-short",
      "dimensions",
      "event",
      "event-place",
      "genre",
      "keyword",
      "medium",
      "original-publisher",
      "original-publisher-place",
      "original-title",
      "publisher",
      "publisher-place",
      "references",
      "reviewed-title",
      "scale",
      "section",
      "source",
      "status",
      "title",
      "title-short",
      "version",

      -- virtual-only variables
      "year-suffix",

      -- CSL-M
      "available-date",
      "volume-title",
      "committee",
      "document-name",
      "gazette-flag"

      -- CSL-M verbatim
      -- "language",
      -- "jurisdiction",

      -- virtual-only variables
      -- "hereinafter",
    ]

parseTextFields :: IO ()
parseTextFields = do
    json <- BL.getContents
    BL.putStr $
        json & values
             . _Object
             . traverseTextFields
             %~ parseInlines

    where
        traverseTextFields = itraversed . indices (`Set.member` textFields)

        -- we'll just silently replace unparsable stuff with []
        emp = A.Array V.empty

        parseInlines :: Value -> Value
        parseInlines (A.String x) = fromRight emp result
            where
                result = runPure $ do
                    doc <- PM.readMarkdown def x
                    let inlines = case doc of
                                    Pandoc meta blocks -> blocksToInlines blocks
                    return $ toJSON inlines
        parseInlines _ = emp
