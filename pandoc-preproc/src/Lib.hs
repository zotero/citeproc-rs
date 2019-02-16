{-# LANGUAGE OverloadedStrings #-}

module Lib
    ( parseTextFields
    ) where

import Text.Pandoc.Definition
import Text.Pandoc.Shared (blocksToInlines)
import Text.Pandoc.Class (runPure)
import Text.Pandoc.Extensions
import Text.Pandoc.Options (ReaderOptions(..))
import Text.Pandoc.Readers.Markdown (readMarkdown)

import qualified Data.Set as Set
import qualified Data.Vector as V
import qualified Text.Pandoc.JSON as PJ
import qualified Data.Text as T
import qualified Data.ByteString as B
import qualified Data.ByteString.Lazy as BL
import qualified Data.Text.Lazy as TL
import Data.Text.Lazy.Encoding (decodeUtf8)

import Data.Default (def)
import Data.Either (fromRight)
import Data.Aeson (toJSON, Value)
import qualified Data.Aeson as A
import Control.Lens
import Data.Aeson.Lens

blockFields :: Set.Set T.Text
blockFields = Set.fromList [ "annote"
                           , "note"
                           , "abstract" ]

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
      -- "year-suffix",
      -- "citation-label",

      -- CSL-M
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
             . itraversed %@~ allFields

    where
        allFields field value
            | field `Set.member` textFields = parseInlines value
            -- | field `Set.member` blockFields = parseBlocks value
            | otherwise                     = value

        -- we'll just silently replace unparsable stuff with []
        emp = A.Array V.empty

        parseInlines :: Value -> Value
        parseInlines (A.String txt) = fromRight emp $ runPure $ do
            Pandoc _ blocks <- readMarkdown (markdownOptions inlineExtensions) txt
            return $ toJSON $ blocksToInlines blocks
        parseInlines _ = emp

        parseBlocks :: Value -> Value
        parseBlocks (A.String txt) = fromRight emp $ runPure $ do
            Pandoc _ blocks <- readMarkdown (markdownOptions blockExtensions) txt
            return $ toJSON $ blocks
        parseBlocks _ = emp

        markdownOptions exts = def { readerExtensions = exts
                                   , readerStandalone = False
                                   , readerStripComments = True }

        inlineExtensions = extensionsFromList
            [ Ext_raw_html
            , Ext_native_spans
            , Ext_native_divs
            , Ext_bracketed_spans -- for smallcaps with [Small caps]{.smallcaps}
            , Ext_tex_math_dollars
            , Ext_backtick_code_blocks
            , Ext_inline_code_attributes
            , Ext_strikeout
            , Ext_superscript
            , Ext_subscript
            , Ext_blank_before_header -- so you can't write #yeah and get a header by accident
            , Ext_all_symbols_escapable
            , Ext_link_attributes
            , Ext_smart
            ]

        blockExtensions
            = disableExtension Ext_inline_notes
            . disableExtension Ext_citations
            . disableExtension Ext_footnotes
            . disableExtension Ext_pandoc_title_block
            . disableExtension Ext_yaml_metadata_block
            $ pandocExtensions
