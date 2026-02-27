<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" id="flat-apps-forms" version="2.0">
  <xsl:output encoding="UTF-8" method="xml"/>
  <xsl:template match="object">
    <xsl:copy>
      <xsl:apply-templates select="@*"/>
      <formation name="Φ" id="0">
        <xsl:for-each select="*[@id and @name]">
          <xsl:element name="attribute">
            <xsl:attribute name="name" select="@name"/>
            <xsl:attribute name="id" select="@id"/>
          </xsl:element>
        </xsl:for-each>
      </formation>
      <xsl:apply-templates select="*"/>
    </xsl:copy>
  </xsl:template>
  <xsl:template match="formation">
    <xsl:copy>
      <xsl:apply-templates select="@*"/>
      <xsl:if test="normalize-space(string-join(text(), '')) != ''">
        <xsl:element name="attribute">
          <xsl:attribute name="name" select="'Δ'"/>
          <xsl:attribute name="value" select="normalize-space(string-join(text(), ''))"/>
        </xsl:element>
      </xsl:if>
      <xsl:for-each select="void">
        <xsl:element name="attribute">
          <xsl:attribute name="name" select="@name"/>
          <xsl:attribute name="void"/>
        </xsl:element>
      </xsl:for-each>
      <xsl:if test="atom">
        <xsl:element name="attribute">
          <xsl:attribute name="name" select="'λ'"/>
          <xsl:attribute name="atom" select="atom/@name"/>
        </xsl:element>
      </xsl:if>
      <xsl:for-each select="*[@id and @name]">
        <xsl:element name="attribute">
          <xsl:attribute name="name" select="@name"/>
          <xsl:attribute name="id" select="@id"/>
        </xsl:element>
      </xsl:for-each>
    </xsl:copy>
    <xsl:apply-templates select="*[name()!='void' and name()!='atom']"/>
  </xsl:template>
  <xsl:template match="application">
    <xsl:copy>
      <xsl:apply-templates select="@*"/>
      <xsl:attribute name="from" select="*[1]/@id"/>
      <xsl:attribute name="arg" select="*[2]/@id"/>
    </xsl:copy>
    <xsl:apply-templates select="*"/>
  </xsl:template>
  <xsl:template match="node()|@*">
    <xsl:copy>
      <xsl:apply-templates select="node()|@*"/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>
