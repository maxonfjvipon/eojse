<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" id="to-dsl" version="2.0">
  <xsl:output method="text" encoding="UTF-8"/>
  <xsl:template match="formation">
    <xsl:text>FORM </xsl:text>
    <xsl:value-of select="@id"/>
    <xsl:text> </xsl:text>
    <xsl:choose>
      <xsl:when test="@name">
        <xsl:value-of select="@name"/>
      </xsl:when>
      <xsl:otherwise>anon</xsl:otherwise>
    </xsl:choose>
    <xsl:for-each select="attribute">
      <xsl:text> </xsl:text>
      <xsl:value-of select="@name"/>
      <xsl:text>:</xsl:text>
      <xsl:choose>
        <xsl:when test="@void">?</xsl:when>
        <xsl:when test="@atom">
          <xsl:value-of select="@atom"/>
        </xsl:when>
        <xsl:when test="@value">
          <xsl:value-of select="@value"/>
        </xsl:when>
        <xsl:when test="@cache">
          <xsl:value-of select="@id"/>
          <xsl:text>!</xsl:text>
        </xsl:when>
        <xsl:otherwise>
          <xsl:value-of select="@id"/>
        </xsl:otherwise>
      </xsl:choose>
    </xsl:for-each>
    <xsl:text>&#10;</xsl:text>
  </xsl:template>
  <xsl:template match="dispatch">
    <xsl:text>DISP </xsl:text>
    <xsl:value-of select="@id"/>
    <xsl:text> </xsl:text>
    <xsl:choose>
      <xsl:when test="@from">
        <xsl:value-of select="@from"/>
      </xsl:when>
      <xsl:otherwise>-1</xsl:otherwise>
    </xsl:choose>
    <xsl:text> </xsl:text>
    <xsl:choose>
      <xsl:when test="@attr">
        <xsl:value-of select="@attr"/>
      </xsl:when>
      <xsl:otherwise>-1</xsl:otherwise>
    </xsl:choose>
    <xsl:text>&#10;</xsl:text>
  </xsl:template>
  <xsl:template match="application">
    <xsl:text>APP </xsl:text>
    <xsl:value-of select="@id"/>
    <xsl:text> </xsl:text>
    <xsl:value-of select="@from"/>
    <xsl:text> </xsl:text>
    <xsl:choose>
      <xsl:when test="starts-with(@as, 'α')">
        <xsl:value-of select="substring-after(@as, 'α')"/>
      </xsl:when>
      <xsl:otherwise>
        <xsl:value-of select="@as"/>
      </xsl:otherwise>
    </xsl:choose>
    <xsl:text> </xsl:text>
    <xsl:value-of select="@arg"/>
    <xsl:if test="@cache">
      <xsl:text> CACHE</xsl:text>
    </xsl:if>
    <xsl:text>&#10;</xsl:text>
  </xsl:template>
  <xsl:template match="node()|@*">
    <xsl:apply-templates select="node()"/>
  </xsl:template>
</xsl:stylesheet>
