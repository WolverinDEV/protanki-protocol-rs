CREATE TABLE `user`(
        `user_id` VARCHAR(32) NOT NULL PRIMARY KEY,
        `email` VARCHAR(128) DEFAULT NULL,
        `email_confirmed` INT DEFAULT 0,

        `timestamp_register` DATETIME NOT NULL,
        `timestamp_active` DATETIME NOT NULL,

        `crystals` INT NOT NULL,
        `double_crystals` DATETIME DEFAULT NULL,

        `experience` INT NOT NULL,
        `premium` DATETIME DEFAULT NULL
);

CREATE TABLE `user_authentication`(
        `user_id` VARCHAR(32) NOT NULL PRIMARY KEY,
        `login_user` VARCHAR(32),
        `password_hash` VARCHAR(64),
        `password_salt` VARCHAR(16),
        FOREIGN KEY(`user_id`) REFERENCES `user`(`user_id`)
);

CREATE TABLE `user_authentication_token`(
        `user_id` VARCHAR(32) NOT NULL PRIMARY KEY,
        `timestamp_created` DATETIME NOT NULL,
        `timestamp_last_used` DATETIME NOT NULL,
        `token` VARCHAR(64),
        FOREIGN KEY(`user_id`) REFERENCES `user`(`user_id`)
);