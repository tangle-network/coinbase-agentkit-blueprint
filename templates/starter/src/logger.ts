import winston from "winston";
import { TransformableInfo } from "logform";

/**
 * Create a logger instance with consistent formatting
 */
export function createLogger() {
  return winston.createLogger({
    level: "info",
    format: winston.format.combine(
      winston.format.timestamp(),
      winston.format.colorize(),
      winston.format.printf((info: TransformableInfo) => {
        const { timestamp, level, message } = info;
        return `${timestamp} ${level}: ${message}`;
      })
    ),
    transports: [
      new winston.transports.Console({
        format: winston.format.combine(
          winston.format.colorize(),
          winston.format.simple()
        ),
      }),
    ],
  });
}

// Export default logger instance
export const logger = createLogger();
