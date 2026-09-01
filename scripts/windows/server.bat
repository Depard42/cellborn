@echo off
chcp 65001 > nul
title Cellborn - сервер
rem Только сервер: слушает UDP 0.0.0.0:5555.
rem Чтобы к нему подключились с других машин, открой этот порт в брандмауэре.
cellborn-server.exe
pause
