-- Your SQL goes here
CREATE TABLE users (
  id INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
  email VARCHAR(100) NOT NULL,
  access_token TEXT,
  password VARCHAR(255) NOT NULL,
  UNIQUE KEY(email)
)
