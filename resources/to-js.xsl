<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" id="remove-xi" version="2.0">
  <xsl:output encoding="UTF-8" method="xml"/>
  <xsl:template match="formation">
    <xsl:text>push(formation('</xsl:text>
    <xsl:if test="@name">
      <xsl:value-of select="@name"/>
    </xsl:if>
    <xsl:if test="not(@name)">
      <xsl:text>anon</xsl:text>
    </xsl:if>
    <xsl:text>', {</xsl:text>
    <xsl:for-each select="attribute">
      <xsl:if test="position()&gt;1">
        <xsl:text>, </xsl:text>
      </xsl:if>
      <xsl:text>'</xsl:text>
      <xsl:value-of select="@name"/>
      <!-- <xsl:text>': </xsl:text> -->
      <xsl:text>': attr(</xsl:text>
      <xsl:if test="@id">
        <xsl:value-of select="@id"/>
      </xsl:if>
      <xsl:if test="@value">
        <xsl:text>[</xsl:text>
        <xsl:variable name="value" select="@value"/>
        <xsl:for-each select="tokenize($value, '-')">
          <xsl:if test="not(ends-with($value, '-')) or position()!=last()">
            <xsl:if test="position()&gt;1">
              <xsl:text>, </xsl:text>
            </xsl:if>
            <xsl:text>'0x</xsl:text>
            <xsl:value-of select="."/>
            <xsl:text>'</xsl:text>
          </xsl:if>
        </xsl:for-each>
        <xsl:text>]</xsl:text>
      </xsl:if>
      <xsl:if test="@void">
        <xsl:text>null</xsl:text>
      </xsl:if>
      <xsl:if test="@atom">
        <xsl:text>'</xsl:text>
        <xsl:value-of select="@atom"/>
        <xsl:text>'</xsl:text>
      </xsl:if>
      <xsl:text>)</xsl:text>
    </xsl:for-each>
    <xsl:text>})) // </xsl:text>
    <xsl:value-of select="@id"/>
  </xsl:template>
  <xsl:template match="dispatch">
    <xsl:text>push(dispatch('</xsl:text>
    <xsl:choose>
      <xsl:when test="@name">
        <xsl:value-of select="@name"/>
      </xsl:when>
      <xsl:otherwise>
        <xsl:value-of select="@from"/>
        <xsl:text>.</xsl:text>
        <xsl:value-of select="@attr"/>
      </xsl:otherwise>
    </xsl:choose>
    <xsl:text>', </xsl:text>
    <xsl:if test="@from">
      <xsl:value-of select="@from"/>
    </xsl:if>
    <xsl:if test="@self">
      <xsl:text>-1</xsl:text>
    </xsl:if>
    <xsl:text>, '</xsl:text>
    <xsl:if test="@attr">
      <xsl:value-of select="@attr"/>
    </xsl:if>
    <xsl:if test="@self">
      <xsl:text>-1</xsl:text>
    </xsl:if>
    <xsl:text>'</xsl:text>
    <xsl:if test="@cache">
      <xsl:text>, true</xsl:text>
    </xsl:if>
    <xsl:text>)) // </xsl:text>
    <xsl:value-of select="@id"/>
  </xsl:template>
  <xsl:template match="application">
    <xsl:text>push(application('</xsl:text>
    <xsl:choose>
      <xsl:when test="@name">
        <xsl:value-of select="@name"/>
      </xsl:when>
      <xsl:otherwise>
        <xsl:value-of select="@from"/>
        <xsl:text>(</xsl:text>
        <xsl:value-of select="@as"/>
        <xsl:text>: </xsl:text>
        <xsl:value-of select="@arg"/>
        <xsl:text>)</xsl:text>
      </xsl:otherwise>
    </xsl:choose>
    <xsl:text>', </xsl:text>
    <xsl:value-of select="@from"/>
    <xsl:text>, </xsl:text>
    <xsl:choose>
      <xsl:when test="starts-with(@as, 'α')">
        <xsl:value-of select="substring-after(@as, 'α')"/>
      </xsl:when>
      <xsl:otherwise>
        <xsl:text>'</xsl:text>
        <xsl:value-of select="@as"/>
        <xsl:text>'</xsl:text>
      </xsl:otherwise>
    </xsl:choose>
    <xsl:text>, </xsl:text>
    <xsl:value-of select="@arg"/>
    <xsl:if test="@cache">
      <xsl:text>, true</xsl:text>
    </xsl:if>
    <xsl:text>)) // </xsl:text>
    <xsl:value-of select="@id"/>
  </xsl:template>
  <xsl:template match="node()|@*">
    <xsl:copy>
      <xsl:apply-templates select="node()|@*"/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>
