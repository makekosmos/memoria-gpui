// ProseMirror/Tiptap node helpers + the verbose fixture documents.
// Kept separate from build-snapshot.mjs to satisfy the 300-line source gate.

export const text = (t, marks) => ({
  type: "text",
  text: t,
  ...(marks ? { marks } : {}),
});

// Bare strings inside content are invalid nodes — wrap them as text.
export const para = (...content) => ({
  type: "paragraph",
  content: content.map((c) => (typeof c === "string" ? text(c) : c)),
});

export const doc = (content) => ({ type: "doc", content });

// The "kitchen sink" note: every block/mark/node the Vue editor produces.
export const kitchenSinkDoc = () =>
  doc([
    { type: "heading", attrs: { level: 1 }, content: [text("Все элементы разметки")] },
    para(
      text("Обычный текст, "),
      text("жирный", [{ type: "bold" }]),
      text(", "),
      text("курсив", [{ type: "italic" }]),
      text(", "),
      text("зачёркнутый", [{ type: "strike" }]),
      text(", "),
      text("моно", [{ type: "code" }]),
      text(", "),
      text("подчёркнутый", [{ type: "underline" }]),
      text(" и "),
      text("ссылка", [{ type: "link", attrs: { href: "https://kosmos.example/" } }]),
      text("."),
    ),
    { type: "heading", attrs: { level: 2 }, content: [text("Списки")] },
    {
      type: "bulletList",
      content: [
        {
          type: "listItem",
          content: [
            para("Первый уровень"),
            {
              type: "bulletList",
              content: [
                {
                  type: "listItem",
                  content: [
                    para("Второй уровень"),
                    {
                      type: "bulletList",
                      content: [
                        { type: "listItem", content: [para("Третий уровень")] },
                      ],
                    },
                  ],
                },
              ],
            },
          ],
        },
        { type: "listItem", content: [para("Второй пункт")] },
      ],
    },
    {
      type: "orderedList",
      attrs: { start: 1 },
      content: [
        { type: "listItem", content: [para("Раз")] },
        { type: "listItem", content: [para("Два")] },
      ],
    },
    {
      type: "taskList",
      content: [
        {
          type: "taskItem",
          attrs: { checked: true },
          content: [para("Сделанная задача")],
        },
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            para("Открытая задача"),
            {
              type: "taskList",
              content: [
                {
                  type: "taskItem",
                  attrs: { checked: false },
                  content: [para("Вложенная подзадача")],
                },
              ],
            },
          ],
        },
      ],
    },
    { type: "heading", attrs: { level: 3 }, content: [text("Код")] },
    {
      type: "codeBlock",
      attrs: { language: "rust" },
      content: [text('fn main() {\n    println!("привет");\n}')],
    },
    {
      type: "codeBlock",
      attrs: { language: "typescript" },
      content: [text("export const answer: number = 42;")],
    },
    {
      type: "codeBlock",
      attrs: { language: "python" },
      content: [text("def ok():\n    return True")],
    },
    {
      type: "blockquote",
      content: [para("Цитата с «ёлочками»,"), para("второй абзац цитаты")],
    },
    { type: "horizontalRule" },
    para(text("Строка с переносом"), { type: "hardBreak" }, text("вторая строка.")),
    {
      type: "image",
      attrs: { src: "fixtures/attachments/kosmos-mark.png", alt: "Логотип", title: "Kosmos" },
    },
    para(),
  ]);

// A legacy dated journal day: each top-level block becomes one diary bubble.
export const journalDayDoc = () =>
  doc([
    para("Утренняя запись: список дел на день"),
    para(text("Мысль: "), text("идеи приходят на ходу", [{ type: "italic" }])),
    {
      type: "taskList",
      content: [
        {
          type: "taskItem",
          attrs: { checked: true },
          content: [para("Разобрать почту")],
        },
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [para("Прогулка 30 минут")],
        },
      ],
    },
  ]);
