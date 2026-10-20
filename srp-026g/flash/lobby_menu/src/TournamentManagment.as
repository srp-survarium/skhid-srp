package
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   import survarium.clan.TournamentWindow;
   
   public dynamic class TournamentManagment extends TournamentWindow
   {
      
      public function TournamentManagment()
      {
         super();
         addFrameScript(0,this.frame1);
         this.__setProp_playersList_TournamentManagment_();
      }
      
      internal function __setProp_playersList_TournamentManagment_() : *
      {
         try
         {
            playersList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         playersList.draggable = false;
         playersList.enabled = true;
         playersList.enableInitCallback = false;
         playersList.focusable = true;
         playersList.itemRendererName = "SrvClanTeamListRenderer";
         playersList.itemRendererInstanceName = "";
         playersList.margin = 2;
         playersList.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         playersList.rowHeight = 25;
         playersList.scrollBar = "SrvScrollBar";
         playersList.scrollYPadding = 0;
         playersList.visible = true;
         playersList.wrapping = "normal";
         try
         {
            playersList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function frame1() : *
      {
         stop();
      }
   }
}

