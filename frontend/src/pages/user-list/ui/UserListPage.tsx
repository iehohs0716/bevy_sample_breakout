import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { listUsers, type User } from "@/entities/user";
import "./UserListPage.css";

export function UserListPage() {
  const [users, setUsers] = useState<User[] | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    listUsers()
      .then(setUsers)
      .catch((error: Error) => setErrorMessage(error.message));
  }, []);

  return (
    <div className="user-list-page">
      <Link to="/">トップに戻る</Link>
      <h1>ユーザー一覧</h1>
      <Link to="/users/new" className="user-list-page__create-link">
        ユーザーを登録する
      </Link>

      {errorMessage && <p className="user-list-page__error">エラー: {errorMessage}</p>}

      {users && users.length === 0 && <p>登録されているユーザーはいません。</p>}

      {users && users.length > 0 && (
        <ul className="user-list">
          {users.map((user) => (
            <li key={user.id} className="user-list__item">
              <Link to={`/users/${user.id}`}>
                <span className="user-list__display-name">{user.displayName}</span>
                <span className="user-list__email">{user.email}</span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
