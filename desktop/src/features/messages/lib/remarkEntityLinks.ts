/** Detect bare Buzz entity URLs in markdown text nodes. */
import { createRemarkPrefixPlugin } from "../../../shared/lib/createRemarkPrefixPlugin.ts";

const ENTITY_URL_PATTERN =
  /buzz:\/\/(?:pr|issue|repo|project)\?[^\s<>"')\]]+|buzz:\/\/mkideas(?:\/entity)?\?[^\s<>"')\]]+/g;
const TRAILING_PUNCTUATION_PATTERN = /[.,;:!?]+$/;

export default function remarkEntityLinks() {
  return createRemarkPrefixPlugin(ENTITY_URL_PATTERN, (matchText) => {
    const value = matchText.replace(TRAILING_PUNCTUATION_PATTERN, "");
    return {
      node: {
        type: "entity-link",
        value,
        data: {
          hName: "entity-link",
          hChildren: [{ type: "text", value }],
        },
      },
      trailing: matchText.slice(value.length),
    };
  });
}
