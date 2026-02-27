<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" id="formation" version="2.0">
  <xsl:output encoding="UTF-8" method="xml"/>
  <xsl:template match="o[not(@base)]">
    <xsl:choose>
      <xsl:when test="@name and @name='λ'">
        <xsl:variable name="fqn" select="concat('L_', string-join(ancestor::o/@name, '_'))"/>
        <xsl:element name="atom">
          <xsl:attribute name="name" select="$fqn"/>
        </xsl:element>
      </xsl:when>
      <xsl:otherwise>
        <xsl:element name="formation">
          <xsl:if test="@name">
            <xsl:attribute name="name" select="@name"/>
          </xsl:if>
          <xsl:apply-templates select="node()"/>
        </xsl:element>
      </xsl:otherwise>
    </xsl:choose>
  </xsl:template>
  <xsl:template match="node()|@*">
    <xsl:copy>
      <xsl:apply-templates select="node()|@*"/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>
