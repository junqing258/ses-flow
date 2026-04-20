import MarkdownIt from "markdown-it";

const markdownRenderer = new MarkdownIt({
  breaks: true,
  html: false,
  linkify: true,
  typographer: true,
});

const defaultLinkOpenRenderer =
  markdownRenderer.renderer.rules.link_open ??
  ((tokens, index, options, _env, self) =>
    self.renderToken(tokens, index, options));

markdownRenderer.renderer.rules.link_open = (
  tokens,
  index,
  options,
  env,
  self,
) => {
  const token = tokens[index];
  token.attrSet("target", "_blank");
  token.attrSet("rel", "noopener noreferrer");
  return defaultLinkOpenRenderer(tokens, index, options, env, self);
};

export const renderAiChatMarkdown = (content: string) =>
  markdownRenderer.render(content).trim();
