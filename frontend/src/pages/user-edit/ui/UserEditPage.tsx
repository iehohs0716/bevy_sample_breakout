import { useEffect, useState, type FormEvent } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { getUser, updateUser } from "@/entities/user";
import "./UserEditPage.css";

export function UserEditPage() {
  const { userId } = useParams<{ userId: string }>();
  const navigate = useNavigate();
  const [displayName, setDisplayName] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isLoaded, setIsLoaded] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    if (!userId) return;
    getUser(userId)
      .then((fetched) => {
        setDisplayName(fetched.displayName);
        setIsLoaded(true);
      })
      .catch((error: Error) => setErrorMessage(error.message));
  }, [userId]);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (!userId) return;
    setErrorMessage(null);
    setIsSaving(true);
    try {
      await updateUser(userId, { displayName });
      navigate(`/users/${userId}`);
    } catch (error) {
      setErrorMessage((error as Error).message);
      setIsSaving(false);
    }
  }

  return (
    <div className="user-edit-page">
      {userId && <Link to={`/users/${userId}`}>ユーザー詳細に戻る</Link>}
      <h1>ユーザー編集</h1>

      {errorMessage && <p className="user-edit-page__error">エラー: {errorMessage}</p>}

      {isLoaded && (
        <form className="user-edit-page__form" onSubmit={handleSubmit}>
          <label>
            表示名
            <input
              type="text"
              required
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
            />
          </label>
          <button type="submit" disabled={isSaving}>
            {isSaving ? "保存中…" : "保存する"}
          </button>
        </form>
      )}
    </div>
  );
}
