import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { deleteUser, getUser, type User } from "@/entities/user";
import "./UserDetailPage.css";

export function UserDetailPage() {
  const { userId } = useParams<{ userId: string }>();
  const navigate = useNavigate();
  const [user, setUser] = useState<User | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);

  useEffect(() => {
    if (!userId) return;
    getUser(userId)
      .then(setUser)
      .catch((error: Error) => setErrorMessage(error.message));
  }, [userId]);

  async function handleDelete() {
    if (!userId) return;
    setErrorMessage(null);
    setIsDeleting(true);
    try {
      await deleteUser(userId);
      navigate("/users");
    } catch (error) {
      setErrorMessage((error as Error).message);
      setIsDeleting(false);
    }
  }

  return (
    <div className="user-detail-page">
      <Link to="/users">ユーザー一覧に戻る</Link>
      <h1>ユーザー詳細</h1>

      {errorMessage && <p className="user-detail-page__error">エラー: {errorMessage}</p>}

      {user && (
        <>
          <dl className="user-detail-page__info">
            <dt>表示名</dt>
            <dd>{user.displayName}</dd>
            <dt>メールアドレス</dt>
            <dd>{user.email}</dd>
            <dt>ID</dt>
            <dd>{user.id}</dd>
            <dt>作成日時</dt>
            <dd>{new Date(user.createdAt).toLocaleString()}</dd>
          </dl>

          <div className="user-detail-page__actions">
            <Link to={`/users/${user.id}/edit`} className="user-detail-page__edit-link">
              編集する
            </Link>
            <button
              type="button"
              className="user-detail-page__delete-button"
              onClick={handleDelete}
              disabled={isDeleting}
            >
              {isDeleting ? "削除中…" : "このユーザーを削除する"}
            </button>
          </div>
        </>
      )}
    </div>
  );
}
