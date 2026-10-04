/*
 * A line of words with the title it is about made to stand out: a notification
 * says what happened to a film, and the film's name is what is looked for.
 */

export function Named({ text, named }: { text: string; named?: string }) {
  const at = named ? text.indexOf(named) : -1;
  if (!named || at < 0) {
    return <>{text}</>;
  }
  return (
    <>
      {text.slice(0, at)}
      <span className="note-name">{named}</span>
      {text.slice(at + named.length)}
    </>
  );
}
