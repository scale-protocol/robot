FROM alpine:3.16
LABEL MAINTAINER="tttlkkkl <tttlkkkl@scale.com>"
ENV TZ "Asia/Shanghai"
COPY app /usr/local/bin/app
WORKDIR /app
EXPOSE 3000
CMD app