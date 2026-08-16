import "./GoogleLoginButton.css";

type GoogleLoginButtonProps = {
  onClick: () => void;
  disabled?: boolean;
  label?: string;
};

export function GoogleLoginButton({
  onClick,
  disabled,
  label = "Googleでログイン",
}: GoogleLoginButtonProps) {
  return (
    <button
      type="button"
      className="google-login-button"
      onClick={onClick}
      disabled={disabled}
    >
      {label}
    </button>
  );
}
