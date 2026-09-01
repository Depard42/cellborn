@echo off
chcp 65001 > nul
title Cellborn
rem Поднимает локальный сервер и подключает к нему клиента.
rem Сервер закрывается сам, когда закрывается игра.

start "Cellborn server" /min cellborn-server.exe
timeout /t 2 /nobreak > nul
cellborn-client.exe %*
taskkill /f /im cellborn-server.exe > nul 2>&1
