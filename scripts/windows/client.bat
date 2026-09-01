@echo off
chcp 65001 > nul
title Cellborn - клиент
rem Только клиент. По умолчанию подключается к 127.0.0.1:5555.
rem К чужому серверу:  client.bat 192.168.1.10:5555
cellborn-client.exe %*
pause
