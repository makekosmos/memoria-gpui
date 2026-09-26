// Typed objects, collections, tasks, diary bubbles and legacy journal days.
// Split from build-snapshot.mjs to satisfy the 300-line source gate.
import { doc, journalDayDoc, para, text } from "./content.mjs";

export function fillRecords(objects, links, note, ts) {
  const typed = (id, memoriaTypeId, title, fields, extra = {}) =>
    note(id, title, doc([para(text(title))]), {
      description: fields.description ?? null,
      extensions: { memoria_type_id: memoriaTypeId, ...fields },
    }, extra);

  objects.push(
    typed("book-1", "book_obj", "Мастер и Маргарита", {
      author: "Михаил Булгаков",
      isbn: "978-5-17-000000-0",
      page_count: 480,
      language: "ru",
      publisher: "АСТ",
      published_date: "1967-01-01",
      source_url: "https://example.invalid/books/master",
      cover_image: "image-1",
    }),
    typed("book-2", "book_obj", "Без обложки", { author: "Неизвестен" }),
    typed("person-1", "system-type-person", "Анна Смирнова", {
      first_name: "Анна",
      last_name: "Смирнова",
      patronymic: "Игоревна",
      birth_date: "1990-04-12",
      photo: "",
    }),
    typed("game-1", "game_obj", "Half-Life 2", {
      user_rating: 5,
      play_status: "completed",
      genres: ["fps", "sci-fi"],
      cover_image: "",
      exe_path: "/games/hl2/hl2.exe",
      total_playtime_seconds: 61200,
      rawg_id: 4291,
    }),
    typed("image-1", "system-type-image", "Обложка", {
      image: "fixtures/attachments/kosmos-mark.png",
      file_name: "kosmos-mark.png",
      mime_type: "image/png",
      size_bytes: 273,
      width: 64,
      height: 64,
      source_path: "fixtures/attachments/kosmos-mark.png",
      alt_text: "Знак Kosmos",
    }),
    typed("workout-1", "system-type-workout", "Утренняя тренировка", {
      date: "2026-09-24",
      duration_min: 45,
      volume_kg: 2400,
      exercise_count: 5,
    }),
    typed("exercise-1", "system-type-exercise", "Приседания", {
      exercise_name: "Приседания",
      exercise_type: "strength",
      muscle_group: "ноги",
    }),
  );

  // Custom note type record + a typed note using it.
  const projectType = {
    id: "project_obj",
    name: "Проект",
    slug: "project",
    icon: "folder",
    color: "#2aa7ee",
    schema_json: JSON.stringify({
      fields: [
        { id: "status", label: "Статус", kind: "select", required: true, options: ["active", "paused", "done"] },
        { id: "repo", label: "Репозиторий", kind: "url", required: false },
        { id: "summary", label: "Описание", kind: "long_text", required: false },
        { id: "owner", label: "Владелец", kind: "relation", required: false, allowed_object_types: ["system-type-person"] },
      ],
    }),
    header_template_json: JSON.stringify({ kind: "default", primaryFieldIds: ["status"], secondaryFieldIds: ["repo"], imageFieldId: null }),
    ui_schema_json: JSON.stringify({
      featured_fields: ["status"],
      visible_fields: ["status", "repo", "summary"],
      hidden_fields: ["created_at", "updated_at", "deleted_at"],
      header_layout: "inline",
      default_layout: "page",
      collection_name: "Проекты",
    }),
    created_at: Date.parse(ts(20)),
    updated_at: Date.parse(ts(24)),
  };
  objects.push(
    note("memoria:type:project_obj", "Проект", doc([]), {
      description: null,
      extensions: { memoria_record_kind: "note_type", note_type_id: "project_obj", memoria_note_type: projectType },
    }),
    typed("project-1", "project_obj", "memoria-gpui", {
      status: "active",
      repo: "https://github.com/makekosmos/memoria-gpui",
      summary: "GPUI-порт Memoria",
      owner: "person-1",
    }),
  );

  // Collections: canonical form seeds; the retired ARK typeId form is kept for
  // reader tests only.
  objects.push(
    note("collection:book_obj", "Книги", doc([]), {
      description: null,
      extensions: { memoria_type_id: "collection_obj", object_type_id: "book_obj" },
    }),
    note("collection:project_obj", "Проекты", doc([]), {
      description: null,
      extensions: { object_type_id: "project_obj" },
    }, { typeId: "collection_obj", $seed: false }),
  );

  // Kepler tasks (com.kosmos.task) — canonical schema requires every field.
  const task = (id, title, props, extra = {}) => ({
    id,
    typeId: "com.kosmos.task",
    typeVersion: "1.0.0",
    title,
    contentJson: doc([para(text(title))]),
    propsJson: {
      status: "todo",
      priority: "none",
      scheduledAt: null,
      dueAt: null,
      reminderAt: null,
      completedAt: null,
      canceledAt: null,
      recurrence: null,
      checklist: [],
      extensions: {},
      ...props,
    },
    createdAt: ts(23),
    updatedAt: ts(24),
    deletedAt: null,
    ...extra,
  });
  objects.push(
    task("task-1", "Прочитать главу 12", {
      status: "todo", priority: "high", dueAt: "2026-09-27T18:00:00.000Z",
      checklist: [{ id: "c1", title: "Купить закладки", isCompleted: true }],
      extensions: { source_note_id: "book-1" },
    }),
    task("task-2", "Закрыть KOS-147", {
      status: "done", completedAt: "2026-09-25T15:30:00.000Z",
    }),
    task("task-3", "Отменённая задача", { status: "canceled", canceledAt: ts(25) }, { deletedAt: ts(25) }),
  );

  // Diary bubbles (com.kosmos.note + extensions.entry_kind=bubble).
  const bubble = (id, textOrDoc, { kind = "plain", tags = [], day = 24, h = 9, m = 0 } = {}) =>
    note(
      id,
      typeof textOrDoc === "string" ? textOrDoc.slice(0, 80) : "Пузырёк",
      typeof textOrDoc === "string" ? doc([para(text(textOrDoc))]) : textOrDoc,
      { description: null, extensions: { entry_kind: "bubble", bubble_kind: kind, tags } },
      { createdAt: ts(day, h, m), updatedAt: ts(day, h, m) },
    );
  objects.push(
    bubble("bubble-1", "Утро: прогнозировать список экранов Memoria", { tags: ["план", "kos-147"], day: 24, h: 9, m: 15 }),
    bubble("bubble-2", "Идея: матрица паритета как единый источник правды", { kind: "idea", tags: ["идея"], day: 24, h: 10, m: 2 }),
    bubble("bubble-3", "Сделать фикстуры до скриншотов", { kind: "task", day: 24, h: 11, m: 40 }),
    bubble("bubble-4", doc([para(text("Важно: "), text("content_json байт-в-байт", [{ type: "bold" }]))]), { kind: "highlight", tags: ["контракт"], day: 25, h: 14, m: 5 }),
    bubble("bubble-5", "Ответ в треде: проверить reply_to", { day: 25, h: 14, m: 30 }),
    bubble("bubble-6", "Ещё один ответ в том же треде", { day: 25, h: 15, m: 1 }),
    bubble("bubble-7", "Вечерний пузырёк без тегов", { day: 26, h: 20, m: 45 }),
  );
  // legacy journal-typed bubble — retired typeId, reader-path only
  objects.push(
    note("bubble-legacy-journal", "Легаси пузырёк (system-type-journal)", doc([para(text("Легаси пузырёк (system-type-journal)"))]), {
      description: null,
      extensions: { entry_kind: "bubble", bubble_kind: "plain", tags: [] },
    }, { createdAt: ts(23, 8, 10), updatedAt: ts(23, 8, 10), typeId: "system-type-journal", $seed: false }),
  );

  // Legacy dated journal entries: the diary view turns each top-level block of a
  // "YYYY-MM-DD"-titled note/journal entry into a timeline bubble. These seed
  // cleanly under the canonical contract (unlike ARK bubble objects, whose flat
  // entry_kind props are rejected by both the launch grant and canonical
  // ingress — see REPORT).
  const journalDay = (id, day) =>
    note(
      id,
      day,
      journalDayDoc(),
      {},
      { createdAt: `${day}T08:00:00.000Z`, updatedAt: `${day}T08:00:00.000Z` },
    );
  objects.push(journalDay("journal-2026-09-24", "2026-09-24"), journalDay("journal-2026-09-25", "2026-09-25"));

  // Links: bubble threads (reply_to) + related notes.
  links.push(
    { id: "bubble-5:reply_to:bubble-3", sourceObjectId: "bubble-5", targetObjectId: "bubble-3", linkType: "reply_to", createdAt: ts(25, 14, 30) },
    { id: "bubble-6:reply_to:bubble-3", sourceObjectId: "bubble-6", targetObjectId: "bubble-3", linkType: "reply_to", createdAt: ts(25, 15, 1) },
    { id: "lnk-related-1", sourceObjectId: "note-markup-kitchen-sink", targetObjectId: "project-1", linkType: "related", createdAt: ts(24) },
    { id: "lnk-related-2", sourceObjectId: "book-1", targetObjectId: "note-markup-kitchen-sink", linkType: "related", createdAt: ts(24) },
  );

}
